use serde::{Deserialize, Serialize};

/// How an observation was judged by the database's validators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Validation {
    #[default]
    Good,
    Discrepant,
    Bad,
}

/// One brightness measurement of a star.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    /// Julian Date of the observation.
    pub jd: f64,
    /// Magnitude (larger is fainter).
    pub mag: f64,
    /// One-sigma uncertainty in magnitudes; 0 when unknown.
    pub error: f64,
    /// Band short name, e.g. "V", "Vis.", "CV" (see [`crate::bands`]).
    pub band: String,
    /// The star was fainter than `mag` (a "<" observation).
    pub fainter_than: bool,
    /// The observer marked the estimate as uncertain (":").
    pub uncertain: bool,
    pub validation: Validation,
    pub observer: Option<String>,
    pub obs_type: Option<String>,
    pub comment: Option<String>,
}

impl Observation {
    pub fn new(jd: f64, mag: f64, band: impl Into<String>) -> Self {
        Self {
            jd,
            mag,
            error: 0.0,
            band: band.into(),
            fainter_than: false,
            uncertain: false,
            validation: Validation::Good,
            observer: None,
            obs_type: None,
            comment: None,
        }
    }
}

/// What a catalogue (e.g. AAVSO VSX) knows about a star.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StarInfo {
    pub name: String,
    pub auid: Option<String>,
    pub period_days: Option<f64>,
    pub epoch_jd: Option<f64>,
    pub var_type: Option<String>,
    pub spectral_type: Option<String>,
    pub ra_deg: Option<f64>,
    pub dec_deg: Option<f64>,
    pub obs_count: Option<u64>,
}

/// A request for observations of one star.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StarQuery {
    pub name: String,
    pub min_jd: Option<f64>,
    pub max_jd: Option<f64>,
    /// Band short names to request; empty means all bands.
    pub bands: Vec<String>,
}

impl StarQuery {
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }
}

/// A star plus its loaded observations, sorted by JD.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Dataset {
    pub star: StarInfo,
    /// Human-readable description of where the data came from.
    pub source: String,
    pub observations: Vec<Observation>,
}

impl Dataset {
    pub fn sort_by_jd(&mut self) {
        self.observations.sort_by(|a, b| a.jd.total_cmp(&b.jd));
    }

    /// The distinct bands present, with how many observations each has,
    /// most populous first.
    pub fn band_counts(&self) -> Vec<(String, usize)> {
        let mut counts: Vec<(String, usize)> = Vec::new();
        for ob in &self.observations {
            match counts.iter_mut().find(|(b, _)| *b == ob.band) {
                Some((_, n)) => *n += 1,
                None => counts.push((ob.band.clone(), 1)),
            }
        }
        counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        counts
    }

    pub fn jd_range(&self) -> Option<(f64, f64)> {
        Some((self.observations.first()?.jd, self.observations.last()?.jd))
    }
}

/// Parse a magnitude string such as "8.4", "<12.5" or "9.1:".
///
/// Returns `(magnitude, fainter_than, uncertain)`.
pub fn parse_magnitude(s: &str) -> Option<(f64, bool, bool)> {
    let mut s = s.trim();
    let fainter = s.starts_with('<');
    if fainter {
        s = s[1..].trim_start();
    }
    let uncertain = s.ends_with(':');
    if uncertain {
        s = s[..s.len() - 1].trim_end();
    }
    let mag = s.parse::<f64>().ok().filter(|m| m.is_finite())?;
    Some((mag, fainter, uncertain))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_magnitude_modifiers() {
        assert_eq!(parse_magnitude("8.4"), Some((8.4, false, false)));
        assert_eq!(parse_magnitude("<12.5"), Some((12.5, true, false)));
        assert_eq!(parse_magnitude(" 9.1: "), Some((9.1, false, true)));
        assert_eq!(parse_magnitude("abc"), None);
        assert_eq!(parse_magnitude(""), None);
    }

    #[test]
    fn counts_bands_most_populous_first() {
        let ds = Dataset {
            observations: vec![
                Observation::new(1.0, 8.0, "V"),
                Observation::new(2.0, 8.0, "Vis."),
                Observation::new(3.0, 8.0, "V"),
            ],
            ..Default::default()
        };
        assert_eq!(ds.band_counts(), vec![("V".into(), 2), ("Vis.".into(), 1)]);
    }
}
