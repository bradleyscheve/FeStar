//! Gaia DR3 epoch photometry, from the ESA Gaia archive.
//!
//! The star is found by position (from VSX) with an ADQL cone search on the
//! archive's TAP service, and its individual G, BP and RP measurements are
//! then fetched from the archive's data server. Epoch photometry exists only
//! for stars Gaia flagged as variable (about 10 million sources).
//!
//! The bulk files at <https://cdn.gea.esac.esa.int/Gaia/> hold the same data
//! for the whole sky, which is too large to search by star name.

use super::aavso;
use super::encode_query_value;
use crate::bands;
use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::model::{Dataset, Observation, StarInfo};

const TAP_URL: &str = "https://gea.esac.esa.int/tap-server/tap/sync";
const DATA_URL: &str = "https://gea.esac.esa.int/data-server/data";

/// Gaia times are barycentric JD minus this offset (days).
pub const TIME_OFFSET: f64 = 2_455_197.5;

/// Search radius around the catalogue position, in arcseconds.
const SEARCH_RADIUS_ARCSEC: f64 = 5.0;

/// 2.5 / ln(10): converts a relative flux error into a magnitude error.
const MAG_PER_FLUX: f64 = 1.085_736_204_758_129_6;

/// A Gaia source id from text such as `4066351347261321472` or
/// `Gaia DR3 4066351347261321472`.
pub fn parse_source_id(text: &str) -> Option<u64> {
    let t = text.trim();
    let t = t
        .strip_prefix_ignore_case("gaia")
        .map_or(t, |r| r.trim_start());
    let t = t
        .strip_prefix_ignore_case("dr3")
        .map_or(t, |r| r.trim_start());
    let t = t.trim_start_matches(['-', ':']).trim();
    if t.len() >= 10 && t.bytes().all(|b| b.is_ascii_digit()) {
        t.parse().ok()
    } else {
        None
    }
}

trait StripPrefixIgnoreCase {
    fn strip_prefix_ignore_case(&self, prefix: &str) -> Option<&str>;
}

impl StripPrefixIgnoreCase for str {
    fn strip_prefix_ignore_case(&self, prefix: &str) -> Option<&str> {
        let head = self.get(..prefix.len())?;
        head.eq_ignore_ascii_case(prefix)
            .then(|| &self[prefix.len()..])
    }
}

/// The ADQL cone search that finds Gaia sources near a position.
pub fn cone_search_url(ra_deg: f64, dec_deg: f64) -> String {
    let query = format!(
        "SELECT TOP 10 source_id, phot_g_mean_mag, has_epoch_photometry, \
         DISTANCE(POINT('ICRS', ra, dec), POINT('ICRS', {ra_deg}, {dec_deg})) AS dist \
         FROM gaiadr3.gaia_source \
         WHERE 1 = CONTAINS(POINT('ICRS', ra, dec), CIRCLE('ICRS', {ra_deg}, {dec_deg}, {r})) \
         ORDER BY dist ASC",
        r = SEARCH_RADIUS_ARCSEC / 3600.0
    );
    format!(
        "{TAP_URL}?REQUEST=doQuery&LANG=ADQL&FORMAT=csv&QUERY={}",
        encode_query_value(&query)
    )
}

/// Where a source's epoch photometry can be downloaded as CSV.
pub fn epoch_photometry_url(source_id: u64) -> String {
    format!(
        "{DATA_URL}?ID=Gaia+DR3+{source_id}&RETRIEVAL_TYPE=EPOCH_PHOTOMETRY&FORMAT=csv&DATA_STRUCTURE=INDIVIDUAL&USE_ZIP_ALWAYS=false"
    )
}

fn csv_reader(text: &str) -> csv::Reader<&[u8]> {
    csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(text.as_bytes())
}

fn truthy(s: &str) -> bool {
    matches!(s.to_ascii_lowercase().as_str(), "true" | "t" | "1")
}

/// Pick the closest source that has epoch photometry from a cone-search
/// response (CSV, nearest first).
pub fn pick_source(csv_text: &str) -> Result<u64> {
    let mut rdr = csv_reader(csv_text);
    let headers = rdr
        .headers()
        .map_err(|e| Error::Parse(e.to_string()))?
        .clone();
    let col = |name: &str| headers.iter().position(|h| h.eq_ignore_ascii_case(name));
    let (Some(id_col), Some(flag_col)) = (col("source_id"), col("has_epoch_photometry")) else {
        return Err(Error::Parse(
            "the Gaia archive's answer had no source_id column".into(),
        ));
    };
    let mut nearest: Option<(String, String)> = None;
    for record in rdr.records().flatten() {
        if nearest.is_none() {
            nearest = Some((
                record.get(id_col).unwrap_or("?").to_string(),
                record.get(flag_col).unwrap_or("?").to_string(),
            ));
        }
        let has = record.get(flag_col).is_some_and(truthy);
        if let (true, Some(id)) = (has, record.get(id_col).and_then(|s| s.parse().ok())) {
            return Ok(id);
        }
    }
    Err(Error::Analysis(match nearest {
        Some((id, flag)) => format!(
            "Gaia DR3 {id} is at this star's position, but the archive says it has no epoch photometry (has_epoch_photometry = {flag}). Only stars Gaia classified as variable have it, and very bright stars are often missing."
        ),
        None => "No Gaia DR3 source was found at this star's position.".into(),
    }))
}

/// Parse the data server's epoch-photometry CSV into observations.
pub fn parse_epoch_photometry(csv_text: &str) -> Result<Vec<Observation>> {
    // Skip comment lines and anything before the header row.
    let start = csv_text
        .lines()
        .scan(0usize, |pos, line| {
            let at = *pos;
            *pos += line.len() + 1;
            Some((at, line))
        })
        .find(|(_, l)| {
            let l = l.to_ascii_lowercase();
            !l.starts_with('#') && l.contains("band") && l.contains("time") && l.contains("mag")
        })
        .map_or(0, |(at, _)| at);
    let csv_text = csv_text.get(start..).unwrap_or(csv_text);
    let mut rdr = csv_reader(csv_text);
    let headers = rdr
        .headers()
        .map_err(|e| Error::Parse(e.to_string()))?
        .clone();
    let col = |name: &str| headers.iter().position(|h| h.eq_ignore_ascii_case(name));
    let (Some(band_col), Some(time_col), Some(mag_col)) = (col("band"), col("time"), col("mag"))
    else {
        let head: String = csv_text.chars().take(300).collect();
        return Err(Error::Parse(format!(
            "the Gaia epoch photometry had no band, time and mag columns. The archive sent: {head}"
        )));
    };
    let foe_col = col("flux_over_error");
    let rejected_col = col("rejected_by_photometry");

    let mut out = Vec::new();
    for record in rdr.records().flatten() {
        let band = match record
            .get(band_col)
            .unwrap_or("")
            .to_ascii_uppercase()
            .as_str()
        {
            "G" => bands::GAIA_G,
            "BP" => bands::GAIA_BP,
            "RP" => bands::GAIA_RP,
            _ => continue,
        };
        let (Some(time), Some(mag)) = (
            record.get(time_col).and_then(|s| s.parse::<f64>().ok()),
            record.get(mag_col).and_then(|s| s.parse::<f64>().ok()),
        ) else {
            continue;
        };
        if !time.is_finite() || !mag.is_finite() {
            continue;
        }
        if rejected_col.is_some_and(|c| record.get(c).is_some_and(truthy)) {
            continue;
        }
        let mut obs = Observation::new(time + TIME_OFFSET, mag, band);
        if let Some(foe) = foe_col
            .and_then(|c| record.get(c))
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|f| f.is_finite() && *f > 0.0)
        {
            obs.error = MAG_PER_FLUX / foe;
        }
        obs.observer = Some("Gaia".into());
        out.push(obs);
    }
    if out.is_empty() {
        return Err(Error::Analysis(
            "the Gaia epoch photometry contained no usable measurements".into(),
        ));
    }
    Ok(out)
}

/// Load Gaia DR3 epoch photometry for a star named in VSX, or for a Gaia
/// source id typed directly.
pub fn load(http: &dyn HttpClient, name_or_id: &str) -> Result<Dataset> {
    let (source_id, mut star) = match parse_source_id(name_or_id) {
        Some(id) => (
            id,
            StarInfo {
                name: format!("Gaia DR3 {id}"),
                ..StarInfo::default()
            },
        ),
        None => {
            let star = aavso::fetch_star_info(http, name_or_id)?;
            let (Some(ra), Some(dec)) = (star.ra_deg, star.dec_deg) else {
                return Err(Error::Analysis(
                    "VSX has no position for this star; type its Gaia DR3 source id instead."
                        .into(),
                ));
            };
            let found = http.get_text(&cone_search_url(ra, dec))?;
            (pick_source(&found)?, star)
        }
    };
    let csv_text = http.get_text(&epoch_photometry_url(source_id))?;
    let observations = parse_epoch_photometry(&csv_text)?;
    if star.name.is_empty() {
        star.name = format!("Gaia DR3 {source_id}");
    }
    let mut ds = Dataset {
        star,
        source: format!("Gaia DR3 {source_id}"),
        observations,
    };
    ds.sort_by_jd();
    Ok(ds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_ids() {
        assert_eq!(
            parse_source_id("Gaia DR3 4066351347261321472"),
            Some(4066351347261321472)
        );
        assert_eq!(parse_source_id(" 1234567890123 "), Some(1234567890123));
        assert_eq!(parse_source_id("SS Cyg"), None);
        assert_eq!(parse_source_id("12"), None);
    }

    #[test]
    fn urls() {
        assert!(cone_search_url(10.5, -20.25).contains("gaiadr3.gaia_source"));
        assert!(epoch_photometry_url(42).contains("ID=Gaia+DR3+42"));
    }

    #[test]
    fn picks_nearest_with_epoch_photometry() {
        let text = "source_id,phot_g_mean_mag,has_epoch_photometry,dist\n\
                    1,12.0,False,0.0001\n2,13.0,True,0.0002\n3,9.0,True,0.0003\n";
        assert_eq!(pick_source(text).unwrap(), 2);
        let none = "source_id,phot_g_mean_mag,has_epoch_photometry,dist\n1,12.0,False,0.1\n";
        assert!(
            pick_source(none)
                .unwrap_err()
                .to_string()
                .contains("no epoch")
        );
        let empty = "source_id,phot_g_mean_mag,has_epoch_photometry,dist\n";
        assert!(
            pick_source(empty)
                .unwrap_err()
                .to_string()
                .contains("No Gaia")
        );
    }

    #[test]
    fn parses_epoch_photometry() {
        let text = "source_id,transit_id,band,time,mag,flux,flux_error,flux_over_error,rejected_by_photometry,rejected_by_variability,other_flags,solution_id\n\
                    1,10,G,1000.5,12.0,100.0,1.0,100.0,False,False,0,1\n\
                    1,11,BP,1000.5,12.5,50.0,1.0,50.0,False,False,0,1\n\
                    1,12,G,1001.5,12.1,90.0,1.0,90.0,True,False,0,1\n\
                    1,13,RP,1002.5,,,,,False,False,0,1\n";
        let obs = parse_epoch_photometry(text).unwrap();
        assert_eq!(obs.len(), 2);
        assert_eq!(obs[0].band, "GG");
        assert!((obs[0].jd - (1000.5 + TIME_OFFSET)).abs() < 1e-9);
        assert!((obs[0].error - 0.010857).abs() < 1e-5);
        assert_eq!(obs[1].band, "GBP");
    }

    #[test]
    fn rejects_unexpected_tables() {
        assert!(parse_epoch_photometry("a,b\n1,2\n").is_err());
    }
}
