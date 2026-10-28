//! Phase folding, as in VStar's `PhaseCalcs`, plus a phase-dispersion score
//! used to compare candidate periods.

use super::TimeSeries;
use crate::Observation;

/// Phase in [0, 1) of a time for a period and epoch (time of phase 0).
/// Works for times before the epoch as well as after.
pub fn phase(jd: f64, epoch: f64, period: f64) -> f64 {
    let cycles = (jd - epoch) / period;
    cycles - cycles.floor()
}

/// How the epoch is chosen when the user doesn't type one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EpochStrategy {
    /// Midpoint of the first and last observation (VStar's default).
    #[default]
    MeanOfEnds,
    /// Time of the faintest observation (the minimum of the light curve).
    Minimum,
    /// Time of the brightest observation (the maximum of the light curve).
    Maximum,
}

impl EpochStrategy {
    pub const ALL: [Self; 3] = [Self::MeanOfEnds, Self::Maximum, Self::Minimum];

    pub fn label(self) -> &'static str {
        match self {
            Self::MeanOfEnds => "Mean of first and last JD",
            Self::Maximum => "JD of maximum brightness",
            Self::Minimum => "JD of minimum brightness",
        }
    }

    /// Choose an epoch from the data, or `None` if there is none.
    pub fn epoch(self, ts: &TimeSeries) -> Option<f64> {
        let (first, last) = (*ts.jd.first()?, *ts.jd.last()?);
        match self {
            Self::MeanOfEnds => Some((first + last) / 2.0),
            // Magnitudes are larger when fainter.
            Self::Minimum => ts
                .mag
                .iter()
                .zip(&ts.jd)
                .max_by(|a, b| a.0.total_cmp(b.0))
                .map(|p| *p.1),
            Self::Maximum => ts
                .mag
                .iter()
                .zip(&ts.jd)
                .min_by(|a, b| a.0.total_cmp(b.0))
                .map(|p| *p.1),
        }
    }
}

/// One point of a phase plot. `cycle` is 0 for phase in [0, 1) and -1 for
/// the previous cycle, shown as phase - 1, which is how VStar draws two
/// cycles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhasePoint {
    pub phase: f64,
    pub mag: f64,
    pub index: usize,
}

/// Fold observations at `period` about `epoch`. Returns the standard phase
/// (0 to 1) of each, in the same order as `obs`.
pub fn fold(obs: &[&Observation], epoch: f64, period: f64) -> Vec<PhasePoint> {
    obs.iter()
        .enumerate()
        .map(|(index, o)| PhasePoint {
            phase: phase(o.jd, epoch, period),
            mag: o.mag,
            index,
        })
        .collect()
}

/// Stellingwerf's phase dispersion theta: the variance within phase bins
/// relative to the overall variance. Near 0 means the folded curve is
/// smooth (a good period); near 1 means it is scatter (a poor period).
pub fn pdm_theta(ts: &TimeSeries, period: f64, bins: usize) -> Option<f64> {
    let n = ts.len();
    if n < bins * 2 || period <= 0.0 || bins < 2 {
        return None;
    }
    let mean = ts.mag.iter().sum::<f64>() / n as f64;
    let total_var = ts.mag.iter().map(|m| (m - mean).powi(2)).sum::<f64>() / (n as f64 - 1.0);
    if total_var <= 0.0 {
        return None;
    }
    let epoch = ts.jd[0];
    let mut sum = vec![0.0; bins];
    let mut sum_sq = vec![0.0; bins];
    let mut count = vec![0usize; bins];
    for (t, m) in ts.jd.iter().zip(&ts.mag) {
        let b = ((phase(*t, epoch, period) * bins as f64) as usize).min(bins - 1);
        sum[b] += m;
        sum_sq[b] += m * m;
        count[b] += 1;
    }
    let (mut ss, mut dof) = (0.0, 0.0);
    for b in 0..bins {
        if count[b] > 1 {
            let c = count[b] as f64;
            ss += sum_sq[b] - sum[b] * sum[b] / c;
            dof += c - 1.0;
        }
    }
    (dof > 0.0).then(|| (ss / dof) / total_var)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts() -> TimeSeries {
        let jd: Vec<f64> = (0..200).map(|i| 2_459_000.0 + i as f64 * 0.731).collect();
        let mag = jd
            .iter()
            .map(|t| 9.0 + 0.4 * (std::f64::consts::TAU * (t - 2_459_000.0) / 5.5).sin())
            .collect();
        TimeSeries { jd, mag }
    }

    #[test]
    fn phase_wraps_both_directions() {
        assert!((phase(10.75, 8.0, 1.0) - 0.75).abs() < 1e-12);
        assert!((phase(5.25, 8.0, 1.0) - 0.25).abs() < 1e-12);
        assert_eq!(phase(8.0, 8.0, 3.0), 0.0);
    }

    #[test]
    fn epoch_strategies() {
        let t = TimeSeries {
            jd: vec![1.0, 2.0, 5.0],
            mag: vec![8.0, 6.0, 9.0],
        };
        assert_eq!(EpochStrategy::MeanOfEnds.epoch(&t), Some(3.0));
        assert_eq!(EpochStrategy::Maximum.epoch(&t), Some(2.0));
        assert_eq!(EpochStrategy::Minimum.epoch(&t), Some(5.0));
        assert_eq!(EpochStrategy::Minimum.epoch(&TimeSeries::default()), None);
    }

    #[test]
    fn true_period_scores_better_than_wrong_ones() {
        let t = ts();
        let good = pdm_theta(&t, 5.5, 10).unwrap();
        let bad = pdm_theta(&t, 4.1, 10).unwrap();
        assert!(good < 0.1, "good = {good}");
        assert!(bad > 0.5, "bad = {bad}");
        assert!(pdm_theta(&t, -1.0, 10).is_none());
    }
}
