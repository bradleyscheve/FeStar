//! Julian Date conversions.

/// Convert a UT date such as "2023-05-14", "2023-05-14 21:30:00",
/// "2023-05-14T21:30" or "2023-05-14.896" to a Julian Date.
pub fn parse_ut_to_jd(s: &str) -> Option<f64> {
    let s = s.trim();
    let (date, time) = match s.find(['T', ' ']) {
        Some(i) => (&s[..i], Some(s[i + 1..].trim_end_matches('Z'))),
        None => (s, None),
    };
    let mut parts = date.split(['-', '/']);
    let y: i32 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let day: f64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&m) || !(1.0..32.0).contains(&day) {
        return None;
    }
    let mut frac = 0.0;
    if let Some(t) = time.filter(|t| !t.is_empty()) {
        let mut hms = t.split(':').map(|p| p.parse::<f64>());
        let h = hms.next()?.ok()?;
        let mi = hms.next().transpose().ok()?.unwrap_or(0.0);
        let se = hms.next().transpose().ok()?.unwrap_or(0.0);
        frac = (h + mi / 60.0 + se / 3600.0) / 24.0;
    }
    Some(gregorian_to_jd(y, m, day + frac))
}

/// Meeus, Astronomical Algorithms, chapter 7 (Gregorian calendar).
pub fn gregorian_to_jd(year: i32, month: u32, day: f64) -> f64 {
    let (y, m) = if month <= 2 {
        (year - 1, month + 12)
    } else {
        (year, month)
    };
    let a = (y as f64 / 100.0).floor();
    let b = 2.0 - a + (a / 4.0).floor();
    (365.25 * (y as f64 + 4716.0)).floor() + (30.6001 * (m as f64 + 1.0)).floor() + day + b - 1524.5
}

/// Convert a Julian Date to `(year, month, day, hour, minute, second)` UT
/// (Meeus, chapter 7).
pub fn jd_to_gregorian(jd: f64) -> (i32, u32, u32, u32, u32, u32) {
    let z = (jd + 0.5).floor();
    let f = jd + 0.5 - z;
    let a = if z < 2299161.0 {
        z
    } else {
        let alpha = ((z - 1867216.25) / 36524.25).floor();
        z + 1.0 + alpha - (alpha / 4.0).floor()
    };
    let b = a + 1524.0;
    let c = ((b - 122.1) / 365.25).floor();
    let d = (365.25 * c).floor();
    let e = ((b - d) / 30.6001).floor();
    let day = (b - d - (30.6001 * e).floor()) as u32;
    let month = if e < 14.0 { e - 1.0 } else { e - 13.0 } as u32;
    let year = if month > 2 { c - 4716.0 } else { c - 4715.0 } as i32;
    let secs = (f * 86400.0).round() as u32;
    // Rounding can carry into the next day; keep it simple and clamp.
    let secs = secs.min(86399);
    (year, month, day, secs / 3600, secs / 60 % 60, secs % 60)
}

/// Format a Julian Date as "YYYY-MM-DD HH:MM:SS UT".
pub fn jd_to_ut_string(jd: f64) -> String {
    let (y, mo, d, h, mi, s) = jd_to_gregorian(jd);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02} UT")
}

/// Seconds-since-1970 to Julian Date.
const UNIX_EPOCH_JD: f64 = 2440587.5;

/// The current Julian Date, from the computer's clock.
pub fn now_jd() -> f64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    UNIX_EPOCH_JD + secs / 86400.0
}

/// The Julian Date exactly one calendar year before `jd` (same month, day
/// and time; 29 February becomes 28 February).
pub fn one_year_before(jd: f64) -> f64 {
    let (year, month, day, h, mi, s) = jd_to_gregorian(jd);
    let day = if month == 2 && day == 29 { 28 } else { day };
    let frac = (h as f64 * 3600.0 + mi as f64 * 60.0 + s as f64) / 86400.0;
    gregorian_to_jd(year - 1, month, day as f64 + frac)
}

/// Format a Julian Date as "YYYY-MM-DD" (UT).
pub fn jd_to_date_string(jd: f64) -> String {
    let (y, mo, d, ..) = jd_to_gregorian(jd);
    format!("{y:04}-{mo:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn julian_dates_match_meeus_examples() {
        assert_eq!(gregorian_to_jd(2000, 1, 1.5), 2451545.0);
        assert_eq!(gregorian_to_jd(1957, 10, 4.81), 2436116.31);
        assert_eq!(parse_ut_to_jd("2000-01-01 12:00"), Some(2451545.0));
        assert_eq!(parse_ut_to_jd("2000-01-01T12:00:00Z"), Some(2451545.0));
        assert_eq!(parse_ut_to_jd("2000-01-01.5"), Some(2451545.0));
        assert_eq!(parse_ut_to_jd("not a date"), None);
    }

    #[test]
    fn converts_jd_back_to_dates() {
        assert_eq!(jd_to_gregorian(2451545.0), (2000, 1, 1, 12, 0, 0));
        assert_eq!(jd_to_ut_string(2436116.31), "1957-10-04 19:26:24 UT");
        assert_eq!(jd_to_gregorian(2299160.5), (1582, 10, 15, 0, 0, 0));
    }

    #[test]
    fn one_year_before_keeps_date_and_time() {
        let now = gregorian_to_jd(2026, 10, 27.75);
        assert_eq!(jd_to_date_string(one_year_before(now)), "2025-10-27");
        assert!((one_year_before(now) - (now - 365.0)).abs() < 1e-9);
        let leap = gregorian_to_jd(2024, 2, 29.0);
        assert_eq!(jd_to_date_string(one_year_before(leap)), "2023-02-28");
    }

    #[test]
    fn now_is_after_2026() {
        assert!(now_jd() > gregorian_to_jd(2026, 1, 1.0));
        assert_eq!(jd_to_date_string(2451545.0), "2000-01-01");
    }
}
