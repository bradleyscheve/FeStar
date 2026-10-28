//! The AAVSO spectroscopy database (AVSpec): finding a star's spectra and
//! downloading them.
//!
//! AVSpec has no public programming interface, so this does what the
//! website's own search page does: it fetches the form (which carries a
//! security token), submits it with the star's name and reads the table of
//! results from the page that comes back.

use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::spectra::{self, Spectrum};

pub const SEARCH_URL: &str = "https://apps.aavso.org/avspec/search/";
const ORIGIN: &str = "https://apps.aavso.org";

/// One row of AVSpec's results table.
#[derive(Debug, Clone, PartialEq)]
pub struct SpectrumRow {
    pub id: u64,
    pub star: String,
    pub observer: String,
    /// Observation date, `YYYY-MM-DD`.
    pub date: String,
    pub jd: f64,
    pub exposure_s: Option<f64>,
    pub resolution: Option<f64>,
}

/// A result row together with the spectrum it points to.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub row: SpectrumRow,
    pub spectrum: Spectrum,
}

/// The security token in AVSpec's search form.
pub fn parse_csrf(html: &str) -> Option<String> {
    let at = html.find("name=\"csrfmiddlewaretoken\"")?;
    let rest = &html[at..];
    let value = rest.find("value=\"")? + "value=\"".len();
    let end = rest[value..].find('"')?;
    Some(rest[value..value + end].to_string())
}

fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn number(s: &str) -> Option<f64> {
    s.trim().parse().ok()
}

/// Read the table of results from the page AVSpec returns, newest first.
pub fn parse_results(html: &str) -> Vec<SpectrumRow> {
    let mut rows = Vec::new();
    for chunk in html.split("<tr").skip(1) {
        let Some(at) = chunk.find("name=\"specids\"") else {
            continue;
        };
        let after = &chunk[at..];
        let Some(value) = after.find("value=") else {
            continue;
        };
        let id: String = after[value + "value=".len()..]
            .trim_start_matches(['"', ' '])
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        let Ok(id) = id.parse::<u64>() else {
            continue;
        };
        let row_html = chunk.split("</tr>").next().unwrap_or(chunk);
        let cells: Vec<String> = row_html
            .split("<td")
            .skip(1)
            .map(|c| {
                let inner = c.split("</td>").next().unwrap_or(c);
                strip_tags(inner.split_once('>').map_or(inner, |(_, rest)| rest))
            })
            .collect();
        // Checkbox, number, star, RA, Dec, observer, date, JD, exposure,
        // resolution, then the View and Download buttons.
        if cells.len() < 10 {
            continue;
        }
        let Some(jd) = number(&cells[7]) else {
            continue;
        };
        rows.push(SpectrumRow {
            id,
            star: cells[2].clone(),
            observer: cells[5].clone(),
            date: cells[6].clone(),
            jd,
            exposure_s: number(&cells[8]),
            resolution: number(&cells[9]),
        });
    }
    rows.sort_by(|a, b| b.jd.total_cmp(&a.jd));
    rows
}

/// Search AVSpec for spectra of the star called `name`, newest first.
pub fn search(http: &dyn HttpClient, name: &str) -> Result<Vec<SpectrumRow>> {
    let page = http.get_text(SEARCH_URL)?;
    let token = parse_csrf(&page).ok_or_else(|| {
        Error::Analysis(
            "AVSpec's search page had no search form. It may be asking you to sign in to AAVSO."
                .into(),
        )
    })?;
    let fields = [
        ("csrfmiddlewaretoken", token.as_str()),
        ("objname", name.trim()),
        ("ra2000", ""),
        ("dec2000", ""),
        ("sradius", ""),
        ("datestart", ""),
        ("dateend", ""),
        ("jdstart", ""),
        ("jdend", ""),
        ("itrplo", ""),
        ("itrphi", ""),
        ("obscode", ""),
    ];
    let html = http.post_form_with_headers(
        SEARCH_URL,
        &fields,
        &[("Referer", SEARCH_URL), ("Origin", ORIGIN)],
    )?;
    Ok(parse_results(&html))
}

/// A short description of a reply that isn't a FITS file, for error messages.
fn describe_reply(bytes: &[u8]) -> String {
    if bytes.starts_with(b"PK") {
        return "a zip file with no FITS spectrum in it".into();
    }
    let text = String::from_utf8_lossy(&bytes[..bytes.len().min(6000)]);
    if text.contains("<html") || text.contains("<!DOCTYPE") || text.contains("<!doctype") {
        let title = text
            .split("<title>")
            .nth(1)
            .and_then(|t| t.split("</title>").next())
            .map(strip_tags)
            .unwrap_or_default();
        let login = text.contains("login") || text.contains("Log in") || text.contains("Sign in");
        return format!(
            "a web page titled \"{title}\"{}",
            if login {
                " that mentions logging in"
            } else {
                ""
            }
        );
    }
    format!("{} bytes of something else", bytes.len())
}

/// The FITS file in a reply: the reply itself, or the first FITS file inside
/// a zip (AVSpec sends its downloads zipped).
fn fits_from(bytes: Vec<u8>) -> std::result::Result<Vec<u8>, Vec<u8>> {
    use std::io::Read;
    if bytes.starts_with(b"SIMPLE") {
        return Ok(bytes);
    }
    if !bytes.starts_with(b"PK") {
        return Err(bytes);
    }
    let found = (|| {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).ok()?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).ok()?;
            let mut data = Vec::new();
            if !entry.is_dir()
                && entry.read_to_end(&mut data).is_ok()
                && data.starts_with(b"SIMPLE")
            {
                return Some(data);
            }
        }
        None
    })();
    found.ok_or(bytes)
}

/// Download one spectrum's FITS file. AVSpec's own Download button submits
/// the search form with the spectrum's id, so that is tried if the direct
/// address doesn't give a FITS file.
pub fn download(http: &dyn HttpClient, id: u64) -> Result<Vec<u8>> {
    let mut problems = Vec::new();
    match http.get_bytes(&spectra::download_url(id)).map(fits_from) {
        Ok(Ok(fits)) => return Ok(fits),
        Ok(Err(bytes)) => problems.push(format!(
            "the direct address gave {}",
            describe_reply(&bytes)
        )),
        Err(e) => problems.push(format!("the direct address failed ({e})")),
    }
    let page = http.get_text(SEARCH_URL)?;
    if let Some(token) = parse_csrf(&page) {
        let id_text = id.to_string();
        let fields = [
            ("csrfmiddlewaretoken", token.as_str()),
            ("specids", id_text.as_str()),
        ];
        match http
            .post_form_bytes_with_headers(
                SEARCH_URL,
                &fields,
                &[("Referer", SEARCH_URL), ("Origin", ORIGIN)],
            )
            .map(fits_from)
        {
            Ok(Ok(fits)) => return Ok(fits),
            Ok(Err(bytes)) => {
                problems.push(format!("the search form gave {}", describe_reply(&bytes)))
            }
            Err(e) => problems.push(format!("the search form failed ({e})")),
        }
    }
    Err(Error::Analysis(format!(
        "couldn't download spectrum {id} from AVSpec: {}",
        problems.join("; ")
    )))
}

/// The `count` most recent spectra of a star after the first `skip`,
/// downloaded and read. Fewer come back when AVSpec has no more to list.
pub fn latest(http: &dyn HttpClient, name: &str, skip: usize, count: usize) -> Result<Vec<Record>> {
    let rows = search(http, name)?;
    if rows.is_empty() {
        return Err(Error::Analysis(format!(
            "AVSpec has no spectra of {}.",
            name.trim()
        )));
    }
    rows.into_iter()
        .skip(skip)
        .take(count)
        .map(|row| {
            let bytes = download(http, row.id)?;
            let spectrum = spectra::parse_fits(&bytes, row.id)?;
            Ok(Record { row, spectrum })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORM: &str = r#"<form action="" method="post"><input type="hidden" name="csrfmiddlewaretoken" value="TOKEN123"><input type="text" name="objname" id="id_objname">"#;

    fn result_row(id: u64, jd: &str, date: &str) -> String {
        format!(
            r#"<tr>
            <td><input type="checkbox" id=spdl1 name="specids"  value={id}  onclick="downloadable()"></td>
            <td> 1 </td><td> del Cep </td><td>22:29:10</td><td>+58:24:54</td>
            <td> BETB </td><td> {date} </td><td> {jd} </td><td>600.0</td><td>
               150.0
            </td>
            <td><a href="/avspec/obs/{id}"><button>View</button></a></td>
            <td><button type="submit" name="specids"  value={id}>Download</button></td>
            </tr>"#
        )
    }

    #[test]
    fn reads_the_security_token() {
        assert_eq!(parse_csrf(FORM).as_deref(), Some("TOKEN123"));
        assert_eq!(parse_csrf("<html>"), None);
    }

    #[test]
    fn reads_results_newest_first() {
        let html = format!(
            "{FORM}<table><tr><th>Star</th></tr>{}{}</table>",
            result_row(100, "2460000.5", "2023-02-25"),
            result_row(200, "2460969.88", "2025-10-21"),
        );
        let rows = parse_results(&html);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, 200);
        assert_eq!(rows[0].star, "del Cep");
        assert_eq!(rows[0].observer, "BETB");
        assert_eq!(rows[0].date, "2025-10-21");
        assert_eq!(rows[0].exposure_s, Some(600.0));
        assert_eq!(rows[0].resolution, Some(150.0));
        assert_eq!(rows[1].id, 100);
    }

    struct FakeAvspec {
        results: String,
    }

    impl HttpClient for FakeAvspec {
        fn get_text(&self, url: &str) -> Result<String> {
            assert_eq!(url, SEARCH_URL);
            Ok(FORM.to_string())
        }
        fn get_bytes(&self, _url: &str) -> Result<Vec<u8>> {
            Ok(b"<html>not a fits file</html>".to_vec())
        }
        fn post_form_with_headers(
            &self,
            _url: &str,
            fields: &[(&str, &str)],
            headers: &[(&str, &str)],
        ) -> Result<String> {
            assert!(fields.contains(&("csrfmiddlewaretoken", "TOKEN123")));
            assert!(fields.contains(&("objname", "del cep")));
            assert!(headers.iter().any(|(k, _)| *k == "Referer"));
            Ok(self.results.clone())
        }
    }

    #[test]
    fn search_submits_the_form() {
        let http = FakeAvspec {
            results: result_row(5, "2460000.5", "2023-02-25"),
        };
        let rows = search(&http, " del cep ").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, 5);
    }

    #[test]
    fn no_results_and_bad_downloads_are_reported() {
        let none = FakeAvspec {
            results: "<html></html>".into(),
        };
        let err = latest(&none, "del cep", 0, 4).unwrap_err();
        assert!(err.to_string().contains("no spectra"));
        let some = FakeAvspec {
            results: result_row(5, "2460000.5", "2023-02-25"),
        };
        let err = latest(&some, "del cep", 0, 4).unwrap_err();
        assert!(err.to_string().contains("couldn't download"));
    }
}

#[cfg(test)]
mod real_page {
    #[test]
    fn real_page_if_present() {
        let Some(path) = std::env::var_os("AVSPEC_PAGE") else {
            return;
        };
        let html = std::fs::read_to_string(path).unwrap();
        let rows = super::parse_results(&html);
        println!("{} rows; first {:?}", rows.len(), &rows[..2]);
        assert_eq!(rows.len(), 25);
        assert_eq!(rows[0].id, 19915);
        assert!(super::parse_csrf(&html).is_some());
    }

    #[test]
    fn reads_a_fits_file_out_of_a_zip() {
        use std::io::Write;
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            zip.start_file("a.fits", zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"SIMPLE  =                    T").unwrap();
            zip.finish().unwrap();
        }
        assert_eq!(
            super::fits_from(buf).unwrap(),
            b"SIMPLE  =                    T"
        );
        assert!(super::fits_from(b"<html>".to_vec()).is_err());
        assert!(super::fits_from(b"SIMPLE".to_vec()).is_ok());
    }
}
