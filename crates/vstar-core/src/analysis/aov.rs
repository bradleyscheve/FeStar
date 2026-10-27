//! Analysis of variance period search, ported from VStar's `AoVPeriodSearch`
//! plug-in: for each trial period, fold the data, split the phase plot into
//! bins and compare the bin means with a one-way ANOVA.

use rayon::prelude::*;

use super::TimeSeries;
use super::binning::symmetric_bins;
use super::phase::{EpochStrategy, phase};
use super::stats::one_way_anova;
use crate::{Error, Result};

/// The most top hits kept (as in VStar).
pub const MAX_TOP_HITS: usize = 20;
/// Refuse searches larger than this many trial periods.
pub const MAX_PERIODS: usize = 2_000_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Params {
    pub min_period: f64,
    pub max_period: f64,
    /// Step between trial periods, in days.
    pub resolution: f64,
    /// Number of phase bins.
    pub bins: usize,
}

impl Params {
    pub fn count(&self) -> usize {
        (((self.max_period - self.min_period) / self.resolution).floor() as usize) + 1
    }

    pub fn validated(self) -> Result<Self> {
        let bad = |m: &str| Err(Error::Analysis(m.into()));
        if !(self.min_period.is_finite() && self.min_period > 0.0) {
            return bad("the minimum period must be greater than zero");
        }
        if !(self.max_period.is_finite() && self.max_period > self.min_period) {
            return bad("the minimum period must be less than the maximum period");
        }
        if !(self.resolution.is_finite() && self.resolution > 0.0) {
            return bad("the resolution must be greater than zero");
        }
        if !(1..=50).contains(&self.bins) {
            return bad("the number of bins must be between 1 and 50");
        }
        if self.count() > MAX_PERIODS {
            return Err(Error::Analysis(format!(
                "that would test {} periods; use a coarser resolution (at most {MAX_PERIODS})",
                self.count()
            )));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub period: f64,
    pub frequency: f64,
    pub f_statistic: f64,
    pub p_value: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Periodogram {
    pub points: Vec<Point>,
}

impl Periodogram {
    /// The best periods, largest F statistic first. Only peaks of the
    /// periodogram count (VStar lists the highest individual points, which
    /// are usually just neighbours on the same peak).
    pub fn top_hits(&self, n: usize) -> Vec<Point> {
        let p = &self.points;
        let f = |i: usize| p[i].f_statistic;
        let mut v: Vec<Point> = (0..p.len())
            .filter(|&i| {
                f(i).is_finite()
                    && (i == 0 || f(i - 1) < f(i) || f(i - 1).is_nan())
                    && (i + 1 == p.len() || f(i) >= f(i + 1) || f(i + 1).is_nan())
            })
            .map(|i| p[i])
            .collect();
        v.sort_by(|a, b| b.f_statistic.total_cmp(&a.f_statistic));
        v.truncate(n);
        v
    }
}

fn evaluate(ts: &TimeSeries, epoch: f64, period: f64, bins: usize) -> Point {
    let mut folded: Vec<(f64, f64)> = ts
        .jd
        .iter()
        .zip(&ts.mag)
        .map(|(&t, &m)| (phase(t, epoch, period), m))
        .collect();
    folded.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (ph, mags): (Vec<f64>, Vec<f64>) = folded.into_iter().unzip();
    let groups: Vec<Vec<f64>> = symmetric_bins(&ph, &mags, 1.0 / bins as f64)
        .into_iter()
        .map(|b| b.mags)
        .collect();
    let (f, p) = one_way_anova(&groups).unwrap_or((f64::NAN, f64::NAN));
    let fix = |v: f64| if v.is_infinite() { f64::NAN } else { v };
    Point {
        period,
        frequency: 1.0 / period,
        f_statistic: fix(f),
        p_value: fix(p),
    }
}

/// Run the search. `progress` receives the fraction done and returns
/// `false` to cancel.
pub fn scan(
    ts: &TimeSeries,
    params: Params,
    progress: &(dyn Fn(f64) -> bool + Sync),
) -> Result<Periodogram> {
    let params = params.validated()?;
    if ts.len() < 4 {
        return Err(Error::Analysis(
            "at least 4 observations are needed for an AoV search".into(),
        ));
    }
    let epoch = EpochStrategy::MeanOfEnds
        .epoch(ts)
        .ok_or_else(|| Error::Analysis("no observations to analyse".into()))?;
    let total = params.count();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let points: Vec<Point> = (0..total)
        .into_par_iter()
        .filter_map(|i| {
            if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                return None;
            }
            let period = params.min_period + i as f64 * params.resolution;
            let point = evaluate(ts, epoch, period, params.bins);
            let d = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if d.is_multiple_of(64) && !progress(d as f64 / total as f64) {
                cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            Some(point)
        })
        .collect();
    if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    Ok(Periodogram { points })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(period: f64) -> TimeSeries {
        let jd: Vec<f64> = (0..300).map(|i| f64::from(i) * 0.731).collect();
        let mag = jd
            .iter()
            .map(|t| 10.0 + (std::f64::consts::TAU * t / period).sin())
            .collect();
        TimeSeries { jd, mag }
    }

    #[test]
    fn finds_the_true_period() {
        let ts = sine(12.5);
        let pg = scan(
            &ts,
            Params {
                min_period: 5.0,
                max_period: 30.0,
                resolution: 0.01,
                bins: 10,
            },
            &|_| true,
        )
        .unwrap();
        let best = pg.top_hits(1)[0];
        assert!((best.period - 12.5).abs() < 0.1, "{best:?}");
        assert!(best.p_value < 1e-6);
    }

    #[test]
    fn rejects_bad_parameters() {
        let p = Params {
            min_period: 5.0,
            max_period: 4.0,
            resolution: 0.1,
            bins: 10,
        };
        assert!(p.validated().is_err());
    }
}
