//! Sending a star's position to programs running on this computer:
//! Stellarium (through its Remote Control plugin) and N.I.N.A. (through the
//! Advanced API plugin, which drives its framing tool).

use serde_json::Value;

use crate::http::HttpClient;
use crate::{Error, Result};

pub const STELLARIUM_DEFAULT_PORT: u16 = 8090;
pub const NINA_DEFAULT_PORT: u16 = 1888;

/// Right ascension as `15h59m30.20s`, the form Stellarium scripts accept.
pub fn ra_hms(ra_deg: f64) -> String {
    let hours = ra_deg.rem_euclid(360.0) / 15.0;
    let mut h = hours.floor();
    let mut m = ((hours - h) * 60.0).floor();
    let mut s = ((hours - h) * 60.0 - m) * 60.0;
    if s >= 59.995 {
        s = 0.0;
        m += 1.0;
    }
    if m >= 60.0 {
        m = 0.0;
        h = (h + 1.0) % 24.0;
    }
    format!("{h:02.0}h{m:02.0}m{s:05.2}s")
}

/// Declination as `+25d55m13.0s`.
pub fn dec_dms(dec_deg: f64) -> String {
    let sign = if dec_deg < 0.0 { '-' } else { '+' };
    let a = dec_deg.abs();
    let mut d = a.floor();
    let mut m = ((a - d) * 60.0).floor();
    let mut s = ((a - d) * 60.0 - m) * 60.0;
    if s >= 59.95 {
        s = 0.0;
        m += 1.0;
    }
    if m >= 60.0 {
        m = 0.0;
        d += 1.0;
    }
    format!("{sign}{d:02.0}d{m:02.0}m{s:04.1}s")
}

/// The Stellarium script that centres the view on a position and then
/// selects the star by name, so a telescope-control plugin can slew to it.
/// If Stellarium doesn't know the name, nothing is selected.
pub fn stellarium_script(ra_deg: f64, dec_deg: f64, name: &str) -> String {
    let mut script = format!(
        "core.moveToRaDecJ2000(\"{}\", \"{}\", 1);\nStelMovementMgr.zoomTo(5, 1);",
        ra_hms(ra_deg),
        dec_dms(dec_deg)
    );
    let name = name.trim().replace(['\\', '"', '\n', '\r'], "");
    if !name.is_empty() {
        script.push_str(&format!(
            "\ncore.wait(1.2);\ncore.selectObjectByName(\"{name}\", true);"
        ));
    }
    script
}

pub fn stellarium_url(port: u16) -> String {
    format!("http://localhost:{port}/api/scripts/direct")
}

/// Centre Stellarium on a position. Needs Stellarium running with its
/// Remote Control plugin switched on.
pub fn open_in_stellarium(
    http: &dyn HttpClient,
    port: u16,
    ra_deg: f64,
    dec_deg: f64,
    name: &str,
) -> Result<()> {
    let script = stellarium_script(ra_deg, dec_deg, name);
    // Stellarium's web server doesn't always read a form body, so the script
    // also goes in the address, which it always reads.
    let code = super::sources::encode_query_value(&script).replace('+', "%20");
    let url = format!("{}?code={code}", stellarium_url(port));
    http.post_form(&url, &[("code", script.as_str())])?;
    Ok(())
}

pub fn nina_url(port: u16, endpoint: &str) -> String {
    format!("http://localhost:{port}/v2/api/{endpoint}")
}

/// The N.I.N.A. Advanced API answers `{"Success": false, "Error": "..."}`
/// when it can't do what was asked.
fn check_nina(reply: &str) -> Result<()> {
    let Ok(json) = serde_json::from_str::<Value>(reply) else {
        return Ok(());
    };
    if json.get("Success").and_then(Value::as_bool) == Some(false) {
        let why = json
            .get("Error")
            .and_then(Value::as_str)
            .unwrap_or("no reason given");
        return Err(Error::Analysis(format!("N.I.N.A. said: {why}")));
    }
    Ok(())
}

/// Open N.I.N.A.'s framing tool at a position. Needs N.I.N.A. running with
/// the Advanced API plugin's server started.
pub fn open_in_nina(http: &dyn HttpClient, port: u16, ra_deg: f64, dec_deg: f64) -> Result<()> {
    check_nina(&http.get_text(&nina_url(port, "application/switch-tab?tab=framing"))?)?;
    let set = format!(
        "framing/set-coordinates?RAangle={}&DecAngle={}",
        ra_deg.rem_euclid(360.0),
        dec_deg
    );
    check_nina(&http.get_text(&nina_url(port, &set))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Recorder {
        calls: Mutex<Vec<String>>,
        reply: String,
    }

    impl HttpClient for Recorder {
        fn get_text(&self, url: &str) -> Result<String> {
            self.calls.lock().unwrap().push(url.to_string());
            Ok(self.reply.clone())
        }
        fn post_form(&self, url: &str, fields: &[(&str, &str)]) -> Result<String> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("{url} {}={}", fields[0].0, fields[0].1));
            Ok(String::new())
        }
    }

    #[test]
    fn formats_coordinates() {
        assert_eq!(ra_hms(239.876), "15h59m30.24s");
        assert_eq!(dec_dms(25.9203), "+25d55m13.1s");
        assert_eq!(dec_dms(-5.5), "-05d30m00.0s");
        assert_eq!(ra_hms(359.9999999), "00h00m00.00s");
    }

    #[test]
    fn stellarium_gets_a_script() {
        let http = Recorder::default();
        open_in_stellarium(&http, 8090, 239.876, 25.9203, "R CrB").unwrap();
        let calls = http.calls.lock().unwrap();
        assert!(
            calls[0].starts_with(
                "http://localhost:8090/api/scripts/direct?code=core.moveToRaDecJ2000%28"
            )
        );
        assert!(calls[0].contains("15h59m30.24s"));
        assert!(calls[0].contains("selectObjectByName%28%22R%20CrB%22"));
    }

    #[test]
    fn stellarium_script_without_a_name_only_centres() {
        let script = stellarium_script(10.0, 20.0, "  ");
        assert!(!script.contains("selectObjectByName"));
        let tricky = stellarium_script(10.0, 20.0, "A\"B\\");
        assert!(tricky.contains("selectObjectByName(\"AB\", true)"));
    }

    #[test]
    fn nina_switches_to_framing_then_sets_coordinates() {
        let http = Recorder {
            reply: r#"{"Success":true}"#.into(),
            ..Recorder::default()
        };
        open_in_nina(&http, 1888, 239.876, 25.9203).unwrap();
        let calls = http.calls.lock().unwrap();
        assert!(calls[0].ends_with("application/switch-tab?tab=framing"));
        assert!(calls[1].contains("framing/set-coordinates?RAangle=239.876&DecAngle=25.9203"));
    }

    #[test]
    fn nina_errors_are_reported() {
        let http = Recorder {
            reply: r#"{"Success":false,"Error":"Framing not ready"}"#.into(),
            ..Recorder::default()
        };
        let err = open_in_nina(&http, 1888, 1.0, 2.0).unwrap_err();
        assert!(err.to_string().contains("Framing not ready"));
    }
}
