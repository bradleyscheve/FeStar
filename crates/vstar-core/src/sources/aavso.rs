//! The AAVSO International Database (AID), via the VSX web service.
//!
//! Ported from VStar's `VSXWebServiceStarInfoSource` and
//! `AIDWebServiceCSV2ObservationSourcePlugin`. The service answers with XML:
//! star details in a `<VSXObject>` element, an optional `<Count>` of matching
//! observations, and the observations themselves as CSV inside `<Data>`.

use roxmltree::Document;

use super::{Progress, encode_query_value};
use crate::bands;
use crate::http::HttpClient;
use crate::model::{Dataset, Observation, StarInfo, StarQuery, Validation, parse_magnitude};
use crate::{Error, Result};

pub const BASE_URL: &str = "https://vsx.aavso.org/index.php?view=api.object";

/// Observations requested per page (same as VStar).
pub const PAGE_SIZE: usize = 50_000;

/// A link to the star's page on VSX, for the "open in browser" button.
pub fn vsx_page_url(star_name: &str) -> String {
    format!(
        "https://vsx.aavso.org/index.php?view=results.get&ident={}",
        encode_query_value(star_name)
    )
}

pub fn star_info_url(star_name: &str) -> String {
    format!("{BASE_URL}&ident={}&data=0", encode_query_value(star_name))
}

pub fn observations_url(ident: &str, query: &StarQuery, page: u32) -> String {
    let mut url = format!(
        "{BASE_URL}&ident={}&data={PAGE_SIZE}",
        encode_query_value(ident)
    );
    if let Some(min) = query.min_jd {
        url.push_str(&format!("&fromjd={min}"));
    }
    if let Some(max) = query.max_jd {
        url.push_str(&format!("&tojd={max}"));
    }
    url.push_str("&csv");
    if !query.bands.is_empty() {
        let bands: Vec<String> = query.bands.iter().map(|b| encode_query_value(b)).collect();
        url.push_str(&format!("&band={}", bands.join(",")));
    }
    url.push_str(&format!("&mtype=std&page={page}"));
    url
}

/// Look a star up in VSX.
pub fn fetch_star_info(http: &dyn HttpClient, star_name: &str) -> Result<StarInfo> {
    let xml = http.get_text(&star_info_url(star_name))?;
    parse_star_info(&xml)?.ok_or_else(|| Error::StarNotFound(star_name.to_string()))
}

/// Load a star's details and all its observations matching `query`.
pub fn load(http: &dyn HttpClient, query: &StarQuery, progress: Progress) -> Result<Dataset> {
    let star = fetch_star_info(http, &query.name)?;
    let ident = star.auid.clone().unwrap_or_else(|| star.name.clone());

    let mut observations = Vec::new();
    let mut page = 1;
    loop {
        let xml = http.get_text(&observations_url(&ident, query, page))?;
        let (mut obs, count) = parse_observations(&xml)?;
        let got = obs.len();
        observations.append(&mut obs);

        if !progress(observations.len(), count) {
            return Err(Error::Cancelled);
        }
        // The service includes <Count> when results are paged. Stop on a
        // short or empty page, or once everything counted has arrived.
        let more = match count {
            Some(total) => got == PAGE_SIZE && (observations.len() as u64) < total,
            None => false,
        };
        if !more {
            break;
        }
        page += 1;
    }

    let mut ds = Dataset {
        star,
        source: "AAVSO International Database".into(),
        observations,
    };
    ds.sort_by_jd();
    Ok(ds)
}

fn child_text<'a>(node: roxmltree::Node<'a, 'a>, tag: &str) -> Option<&'a str> {
    node.children()
        .find(|c| c.has_tag_name(tag))
        .and_then(|c| c.text())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn count_of(doc: &Document) -> Option<u64> {
    doc.descendants()
        .find(|n| n.has_tag_name("Count"))
        .and_then(|n| n.text())
        .and_then(|t| t.trim().parse().ok())
}

/// Parse a VSX `api.object` response. `Ok(None)` means the star is unknown.
pub fn parse_star_info(xml: &str) -> Result<Option<StarInfo>> {
    let doc = Document::parse(xml)?;
    let Some(obj) = doc.descendants().find(|n| n.has_tag_name("VSXObject")) else {
        return Ok(None);
    };
    let Some(name) = child_text(obj, "Name") else {
        return Ok(None);
    };
    let num = |tag| child_text(obj, tag).and_then(|s| s.parse::<f64>().ok());
    let text = |tag| child_text(obj, tag).map(str::to_string);

    Ok(Some(StarInfo {
        name: name.to_string(),
        auid: text("AUID"),
        period_days: num("Period"),
        epoch_jd: num("Epoch"),
        var_type: text("VariabilityType"),
        spectral_type: text("SpectralType"),
        ra_deg: num("RA2000"),
        dec_deg: num("Declination2000"),
        obs_count: count_of(&doc),
    }))
}

/// Parse one page of observations; also returns `<Count>` if present.
pub fn parse_observations(xml: &str) -> Result<(Vec<Observation>, Option<u64>)> {
    let doc = Document::parse(xml)?;
    let count = count_of(&doc);
    let Some(data) = doc
        .descendants()
        .find(|n| n.has_tag_name("Data"))
        .and_then(|n| n.text())
    else {
        return Ok((Vec::new(), count));
    };
    Ok((parse_aid_csv(data)?, count))
}

/// Parse the AID CSV block. Rows without a usable JD or magnitude are
/// skipped, as VStar does.
pub fn parse_aid_csv(data: &str) -> Result<Vec<Observation>> {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(data.trim().as_bytes());
    let headers = rdr.headers()?.clone();
    let col = |name: &str| headers.iter().position(|h| h.eq_ignore_ascii_case(name));
    let (jd_i, mag_i) = match (col("JD"), col("mag")) {
        (Some(j), Some(m)) => (j, m),
        _ => {
            return Err(Error::Parse(
                "AID data is missing the JD or mag column".into(),
            ));
        }
    };
    let (unc_i, ft_i, band_i, by_i, type_i, val_i, com_i) = (
        col("uncert"),
        col("fainterThan"),
        col("band"),
        col("by"),
        col("obstype"),
        col("val"),
        col("comment"),
    );

    let mut out = Vec::new();
    for rec in rdr.records() {
        let rec = rec?;
        let get = |i: Option<usize>| i.and_then(|i| rec.get(i)).filter(|s| !s.is_empty());
        let Some(jd) = rec.get(jd_i).and_then(|s| s.parse::<f64>().ok()) else {
            continue;
        };
        let Some((mag, ft_prefix, uncertain)) = rec.get(mag_i).and_then(parse_magnitude) else {
            continue;
        };

        let mut ob = Observation::new(
            jd,
            mag,
            bands::normalize(get(band_i).unwrap_or(bands::UNKNOWN)),
        );
        ob.error = get(unc_i).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        ob.fainter_than = ft_prefix || get(ft_i) == Some("1");
        ob.uncertain = uncertain;
        ob.validation = validation_from_flag(get(val_i).unwrap_or("V"));
        ob.observer = get(by_i).map(str::to_string);
        ob.obs_type = get(type_i).map(str::to_string);
        ob.comment = get(com_i).map(str::to_string);
        out.push(ob);
    }
    Ok(out)
}

/// AID validation flags: V, Z, U are good; T, N discrepant; anything else
/// (Y, Q) is bad. Same mapping as VStar.
pub fn validation_from_flag(flag: &str) -> Validation {
    match flag.trim() {
        "V" | "Z" | "U" => Validation::Good,
        "T" | "N" => Validation::Discrepant,
        _ => Validation::Bad,
    }
}
