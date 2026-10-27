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
pub const DEFAULT_FOV: u32 = 60;
/// Faintest comparison star magnitude to label.
pub const DEFAULT_MAG_LIMIT: f64 = 14.5;

#[derive(Debug, Clone, PartialEq)]
pub struct Chart {
    pub chart_id: String,
    pub image_url: String,
    /// The chart's page on the VSP website.
    pub page_url: String,
}

pub fn chart_request_url(star_name: &str, fov: u32, mag_limit: f64) -> String {
    format!(
        "{API_URL}?format=json&star={}&fov={fov}&maglimit={mag_limit}",
        encode_query_value(star_name)
    )
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
        .map(str::to_string)
        .unwrap_or_else(|| format!("https://apps.aavso.org/vsp/chart/{chart_id}.png?type=chart"));
    let page_url = format!("https://apps.aavso.org/vsp/chart/{chart_id}/");
    Ok(Chart {
        chart_id,
        image_url,
        page_url,
    })
}

/// Ask VSP for a chart of `star_name` and download its image.
pub fn fetch_chart(http: &dyn HttpClient, star_name: &str) -> Result<(Chart, Vec<u8>)> {
    let json = http.get_text(&chart_request_url(
        star_name,
        DEFAULT_FOV,
        DEFAULT_MAG_LIMIT,
    ))?;
    let chart = parse_chart(&json)?;
    let image = http.get_bytes(&chart.image_url)?;
    Ok((chart, image))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_request() {
        assert_eq!(
            chart_request_url("SS Cyg", 60, 14.5),
            "https://apps.aavso.org/vsp/api/chart/?format=json&star=SS+Cyg&fov=60&maglimit=14.5"
        );
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
