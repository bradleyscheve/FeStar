//! AAVSO Variable Star Plotter (VSP): finder charts with comparison stars.
//!
//! The chart service answers a request for a star with a small JSON
//! description that includes the address of the chart image.

use serde_json::Value;

use super::encode_query_value;
use crate::http::HttpClient;
use crate::{Error, Result};

const API_URL: &str = "https://apps.aavso.org/vsp/api/chart/";

/// Field of view of the chart, in arcminutes (the unit VSP uses).
pub const DEFAULT_FOV: f64 = 60.0;
/// The chart sizes offered to the user, in arcminutes.
pub const FOVS: [f64; 8] = [7.5, 15.0, 30.0, 60.0, 120.0, 240.0, 480.0, 900.0];

/// Faintest comparison star magnitude worth asking for at a field of view.
/// Wide charts get crowded, and VSP refuses limits that are too faint for them.
pub fn default_mag_limit(fov: f64) -> f64 {
    match fov {
        f if f <= 60.0 => 14.5,
        f if f <= 120.0 => 13.5,
        f if f <= 240.0 => 12.0,
        f if f <= 480.0 => 10.5,
        _ => 9.0,
    }
}

/// How much brighter to try when VSP answers "bad request".
const MAG_LIMIT_STEP: f64 = 1.0;
const MIN_MAG_LIMIT: f64 = 6.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Chart {
    pub chart_id: String,
    pub image_url: String,
    /// The magnitude limit the chart was made with.
    pub mag_limit: f64,
    /// The chart's page on the VSP website.
    pub page_url: String,
}

pub fn chart_request_url(star_name: &str, fov: f64, mag_limit: f64) -> String {
    format!(
        "{API_URL}?format=json&star={}&fov={fov}&maglimit={mag_limit}",
        encode_query_value(star_name)
    )
}

/// The picture's address when VSP doesn't give a usable one.
fn fallback_image_url(chart_id: &str) -> String {
    format!("https://apps.aavso.org/vsp/chart/{chart_id}.png?type=chart")
}

/// VSP echoes our `format=json` request in the image address, and asking
/// for the picture "as JSON" makes its server fail with a 500. Drop it.
fn without_format_json(url: &str) -> String {
    let Some((base, query)) = url.split_once('?') else {
        return url.to_string();
    };
    let kept: Vec<&str> = query
        .split('&')
        .filter(|p| !p.eq_ignore_ascii_case("format=json"))
        .collect();
    if kept.is_empty() {
        base.to_string()
    } else {
        format!("{base}?{}", kept.join("&"))
    }
}

pub fn parse_chart(json: &str) -> Result<Chart> {
    let v: Value = serde_json::from_str(json)
        .map_err(|e| Error::Network(format!("unreadable answer from VSP: {e}")))?;
    let chart_id = v["chartid"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Error::Network("VSP didn't return a chart for this star".into()))?
        .to_string();
    let image_url = v["image_uri"]
        .as_str()
        .filter(|s| s.starts_with("https://"))
        .map(without_format_json)
        .unwrap_or_else(|| fallback_image_url(&chart_id));
    let page_url = format!("https://apps.aavso.org/vsp/chart/{chart_id}/");
    Ok(Chart {
        chart_id,
        image_url,
        mag_limit: v["maglimit"].as_f64().unwrap_or(0.0),
        page_url,
    })
}

/// Ask VSP for a chart of `star_name` and download its image. If VSP rejects
/// the request (usually because the magnitude limit is too faint for a wide
/// field), retry with a brighter limit.
pub fn fetch_chart(http: &dyn HttpClient, star_name: &str, fov: f64) -> Result<(Chart, Vec<u8>)> {
    let mut mag_limit = default_mag_limit(fov);
    let json = loop {
        match http.get_text(&chart_request_url(star_name, fov, mag_limit)) {
            Ok(json) => break json,
            Err(Error::Network(m))
                if m.contains("400") && mag_limit - MAG_LIMIT_STEP >= MIN_MAG_LIMIT =>
            {
                mag_limit -= MAG_LIMIT_STEP;
            }
            Err(e) => return Err(e),
        }
    };
    let mut chart = parse_chart(&json)?;
    if chart.mag_limit <= 0.0 {
        chart.mag_limit = mag_limit;
    }
    // The address VSP gave, then the plain chart picture for that id.
    let mut candidates = vec![chart.image_url.clone()];
    for extra in [
        fallback_image_url(&chart.chart_id),
        format!("https://apps.aavso.org/vsp/chart/{}.png", chart.chart_id),
        // The address shown in VSP's own documentation.
        format!(
            "https://www.aavso.org/apps/vsp/chart/{}.png",
            chart.chart_id
        ),
    ] {
        if !candidates.contains(&extra) {
            candidates.push(extra);
        }
    }
    let mut failures = Vec::new();
    for url in candidates {
        // VSP's web service picks its reply format from the Accept header, and
        // answers a request that doesn't ask for a picture with an error.
        match http.get_bytes_with_headers(&url, &[("Accept", "image/png,image/*;q=0.9,*/*;q=0.5")])
        {
            Ok(image) => {
                chart.image_url = url;
                return Ok((chart, image));
            }
            Err(e) => failures.push(format!("{url} ({e})")),
        }
    }
    Err(Error::Network(format!(
        "couldn't download the chart picture. Tried: {}",
        failures.join("; ")
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_format_json_from_image_address() {
        let json = r#"{"chartid":"X43358AGB","image_uri":"https://apps.aavso.org/vsp/chart/X43358AGB.png?format=json"}"#;
        let chart = parse_chart(json).unwrap();
        assert_eq!(
            chart.image_url,
            "https://apps.aavso.org/vsp/chart/X43358AGB.png"
        );
        assert_eq!(
            without_format_json("https://x/y.png?type=chart&format=json"),
            "https://x/y.png?type=chart"
        );
    }

    struct BrokenFirstPicture;
    impl HttpClient for BrokenFirstPicture {
        fn get_text(&self, _: &str) -> Result<String> {
            Ok(r#"{"chartid":"X1","image_uri":"https://apps.aavso.org/vsp/chart/X1.png"}"#.into())
        }
        fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
            if url.ends_with("type=chart") {
                Ok(vec![7])
            } else {
                Err(Error::Network("server answered 500".into()))
            }
        }
    }

    #[test]
    fn falls_back_to_another_picture_address() {
        let (chart, image) = fetch_chart(&BrokenFirstPicture, "U Sco", 60.0).unwrap();
        assert_eq!(image, vec![7]);
        assert!(chart.image_url.ends_with("type=chart"));
    }

    #[test]
    fn builds_request() {
        assert_eq!(
            chart_request_url("SS Cyg", 60.0, 14.5),
            "https://apps.aavso.org/vsp/api/chart/?format=json&star=SS+Cyg&fov=60&maglimit=14.5"
        );
        assert!(chart_request_url("X", 7.5, 14.5).contains("fov=7.5"));
    }

    struct PickyVsp;
    impl HttpClient for PickyVsp {
        fn get_text(&self, url: &str) -> Result<String> {
            let limit: f64 = url.split("maglimit=").nth(1).unwrap().parse().unwrap();
            if limit > 12.0 {
                Err(Error::Network(format!(
                    "server answered 400 Bad Request for {url}"
                )))
            } else {
                Ok(r#"{"chartid":"X1"}"#.into())
            }
        }
        fn get_bytes(&self, _: &str) -> Result<Vec<u8>> {
            Ok(vec![1])
        }
    }

    #[test]
    fn retries_with_brighter_limit_for_every_size() {
        for fov in FOVS {
            let (chart, _) = fetch_chart(&PickyVsp, "T CrB", fov).unwrap();
            assert!(chart.mag_limit <= 12.0, "fov {fov}");
        }
    }

    #[test]
    fn parses_chart_reply() {
        let c = parse_chart(
            r#"{"chartid":"X1","image_uri":"https://apps.aavso.org/vsp/chart/X1.png?type=chart"}"#,
        )
        .unwrap();
        assert_eq!(c.chart_id, "X1");
        assert!(c.image_url.ends_with("X1.png?type=chart"));
        let c = parse_chart(r#"{"chartid":"X2"}"#).unwrap();
        assert_eq!(
            c.image_url,
            "https://apps.aavso.org/vsp/chart/X2.png?type=chart"
        );
        assert!(parse_chart(r#"{"error":"nope"}"#).is_err());
    }
}
