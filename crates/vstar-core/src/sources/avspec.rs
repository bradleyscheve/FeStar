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

/// The `count` most recent spectra of a star, downloaded and read.
pub fn latest(http: &dyn HttpClient, name: &str, count: usize) -> Result<Vec<Record>> {
    let rows = search(http, name)?;
    if rows.is_empty() {
        return Err(Error::Analysis(format!(
            "AVSpec has no spectra of {}.",
            name.trim()
        )));
    }
    rows.into_iter()
        .take(count)
        .map(|row| {
            let bytes = http.get_bytes(&spectra::download_url(row.id))?;
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
        let err = latest(&none, "del cep", 4).unwrap_err();
        assert!(err.to_string().contains("no spectra"));
        let some = FakeAvspec {
            results: result_row(5, "2460000.5", "2023-02-25"),
        };
        let err = latest(&some, "del cep", 4).unwrap_err();
        assert!(err.to_string().contains("FITS"));
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
}
