//! Time-series analysis: period search and phase folding.
//!
//! Ported from AAVSO VStar's `TSDcDft` (the date-compensated discrete
//! Fourier transform from Foster's TS package) and `PhaseCalcs`.

pub mod aov;
pub mod binning;
pub mod dcdft;
pub mod dft;
pub mod models;
pub mod phase;
pub mod stats;
pub mod wwz;

use crate::Observation;

/// Times and magnitudes ready for analysis, sorted by time.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimeSeries {
    pub jd: Vec<f64>,
    pub mag: Vec<f64>,
}

impl TimeSeries {
    /// Build a series from observations, skipping any whose time or
    /// magnitude is not a finite number.
    pub fn from_observations<'a>(obs: impl IntoIterator<Item = &'a Observation>) -> Self {
        let mut pairs: Vec<(f64, f64)> = obs
            .into_iter()
            .filter(|o| o.jd.is_finite() && o.mag.is_finite())
            .map(|o| (o.jd, o.mag))
            .collect();
        pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
        Self {
            jd: pairs.iter().map(|p| p.0).collect(),
            mag: pairs.iter().map(|p| p.1).collect(),
        }
    }

    /// At most `max_points` evenly spaced observations, for quick looks at
    /// very large data sets.
    pub fn thinned(&self, max_points: usize) -> Self {
        if self.len() <= max_points || max_points == 0 {
            return self.clone();
        }
        let step = self.len().div_ceil(max_points);
        let keep = |v: &[f64]| v.iter().step_by(step).copied().collect();
        Self {
            jd: keep(&self.jd),
            mag: keep(&self.mag),
        }
    }

    pub fn len(&self) -> usize {
        self.jd.len()
    }

    pub fn is_empty(&self) -> bool {
        self.jd.is_empty()
    }

    /// Time from the first to the last observation, in days.
    pub fn span(&self) -> f64 {
        match (self.jd.first(), self.jd.last()) {
            (Some(a), Some(b)) => b - a,
            _ => 0.0,
        }
    }

    /// Median gap between consecutive observations that are not on the same
    /// night (more than 0.1 d apart): the typical sampling interval.
    pub fn median_cadence(&self) -> Option<f64> {
        let mut gaps: Vec<f64> = self
            .jd
            .windows(2)
            .map(|w| w[1] - w[0])
            .filter(|g| *g > 0.1)
            .collect();
        if gaps.is_empty() {
            return None;
        }
        gaps.sort_by(f64::total_cmp);
        Some(gaps[gaps.len() / 2])
    }

    /// The longest gap between consecutive observations, in days.
    pub fn longest_gap(&self) -> f64 {
        self.jd.windows(2).map(|w| w[1] - w[0]).fold(0.0, f64::max)
    }

    pub fn mag_range(&self) -> Option<(f64, f64)> {
        let min = self.mag.iter().copied().min_by(f64::total_cmp)?;
        let max = self.mag.iter().copied().max_by(f64::total_cmp)?;
        Some((min, max))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thinning_keeps_order_and_limit() {
        let ts = TimeSeries {
            jd: (0..1000).map(f64::from).collect(),
            mag: vec![1.0; 1000],
        };
        let t = ts.thinned(300);
        assert!(t.len() <= 300 && t.len() >= 250);
        assert!(t.jd.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(ts.thinned(5000), ts);
    }

    #[test]
    fn summarises_sampling() {
        let ts = TimeSeries {
            jd: vec![0.0, 0.01, 1.0, 3.0, 10.0],
            mag: vec![9.0, 9.5, 8.0, 8.5, 9.0],
        };
        assert_eq!(ts.span(), 10.0);
        assert_eq!(ts.longest_gap(), 7.0);
        assert_eq!(ts.median_cadence(), Some(2.0));
        assert_eq!(ts.mag_range(), Some((8.0, 9.5)));
    }
}
