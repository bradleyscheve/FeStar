//! The weighted wavelet Z-transform (Foster 1996), ported from VStar's
//! `WeightedWaveletZTransform`. It finds periodic signals whose period or
//! strength changes with time, by fitting a sinusoid to the data weighted
//! by a Gaussian window that slides along the time axis.

use rayon::prelude::*;

use super::TimeSeries;
use crate::{Error, Result};

const WEIGHT_CUTOFF: f64 = 1.0e-9;
pub const DEFAULT_DECAY: f64 = 0.001;
pub const DEFAULT_TIME_DIVISIONS: f64 = 50.0;
/// The most (time, frequency) cells a search may compute.
pub const MAX_CELLS: usize = 5_000_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Range {
    Frequency { min: f64, max: f64, step: f64 },
    Period { min: f64, max: f64, step: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Params {
    pub range: Range,
    /// Decay constant of the Gaussian window.
    pub decay: f64,
    /// Roughly how many time steps to compute across the data.
    pub time_divisions: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            range: Range::Period {
                min: 1.0,
                max: 100.0,
                step: 1.0,
            },
            decay: DEFAULT_DECAY,
            time_divisions: DEFAULT_TIME_DIVISIONS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stat {
    pub tau: f64,
    pub frequency: f64,
    pub wwz: f64,
    pub semi_amplitude: f64,
    /// The weighted average magnitude.
    pub mean_mag: f64,
    /// The effective number of observations under the window.
    pub neff: f64,
}

impl Stat {
    pub fn period(&self) -> f64 {
        1.0 / self.frequency
    }
}

#[derive(Debug, Clone, Default)]
pub struct Transform {
    /// Every (time, frequency) cell, grouped by time.
    pub stats: Vec<Stat>,
    /// The strongest cell at each time.
    pub maxima: Vec<Stat>,
    pub taus: Vec<f64>,
    pub frequencies: Vec<f64>,
}

impl Transform {
    pub fn max_wwz(&self) -> f64 {
        self.maxima.iter().map(|s| s.wwz).fold(0.0, f64::max)
    }

    /// The WWZ values for time index `i`, in frequency order.
    pub fn row(&self, i: usize) -> &[Stat] {
        let n = self.frequencies.len();
        &self.stats[i * n..(i + 1) * n]
    }
}

/// Round a step size down to 1, 2 or 5 times a power of ten.
fn round_step(x: f64) -> f64 {
    let ex = x.log10().trunc();
    let mut m = x / 10f64.powf(ex);
    m = if m >= 5.0 {
        5.0
    } else if m >= 2.0 {
        2.0
    } else {
        1.0
    };
    m * 10f64.powf(ex)
}

fn make_taus(ts: &TimeSeries, divisions: f64) -> Vec<f64> {
    let (first, last) = (ts.jd[0], ts.jd[ts.len() - 1]);
    let step = round_step((last - first) / divisions);
    let lo = step * ((first / step + 0.5) as i64 as f64);
    let hi = step * ((last / step + 0.5) as i64 as f64);
    let mut taus = Vec::new();
    let mut t = lo;
    while t <= hi {
        taus.push(t);
        t += step;
    }
    taus
}

fn make_frequencies(range: Range) -> Vec<f64> {
    match range {
        Range::Frequency { min, max, step } => {
            let n = ((max - min) / step) as usize + 1;
            (0..n).map(|i| min + i as f64 * step).collect()
        }
        Range::Period { min, max, step } => {
            let n = ((max - min) / step) as usize + 1;
            (0..n).map(|i| 1.0 / (min + i as f64 * step)).collect()
        }
    }
}

fn validate(params: &Params, ts: &TimeSeries) -> Result<()> {
    let bad = |m: &str| Err(Error::Analysis(m.into()));
    let (min, max, step) = match params.range {
        Range::Frequency { min, max, step } | Range::Period { min, max, step } => (min, max, step),
    };
    if !(min.is_finite() && min > 0.0) {
        return bad("the minimum must be greater than zero");
    }
    if !(max.is_finite() && max > min) {
        return bad("the minimum must be less than the maximum");
    }
    if !(step.is_finite() && step > 0.0) {
        return bad("the step must be greater than zero");
    }
    if !(params.decay.is_finite() && params.decay >= 0.0) {
        return bad("the decay constant can't be negative");
    }
    if !(params.time_divisions.is_finite() && params.time_divisions >= 1.0) {
        return bad("the number of time divisions must be at least 1");
    }
    if ts.len() < 4 {
        return bad("at least 4 observations are needed");
    }
    if ts.jd[ts.len() - 1] <= ts.jd[0] {
        return bad("the observations all have the same time");
    }
    Ok(())
}

/// Invert the symmetric 3x3 matrix; unchanged if it's (nearly) singular.
fn invert3(m: &mut [[f64; 3]; 3]) {
    let (a, b, c) = (m[0][0], m[0][1], m[0][2]);
    let (d, e, f) = (m[1][1], m[1][2], m[2][2]);
    let det = a * (d * f - e * e) - b * (b * f - c * e) + c * (b * e - c * d);
    if det.abs() <= 1.0e-14 {
        return;
    }
    let inv = 1.0 / det;
    let a00 = (d * f - e * e) * inv;
    let a01 = (c * e - b * f) * inv;
    let a02 = (b * e - c * d) * inv;
    let a11 = (a * f - c * c) * inv;
    let a12 = (c * b - a * e) * inv;
    let a22 = (a * d - b * b) * inv;
    *m = [[a00, a01, a02], [a01, a11, a12], [a02, a12, a22]];
}

/// One cell of the transform.
#[allow(clippy::needless_range_loop)]
fn cell(ts: &TimeSeries, tau: f64, freq: f64, decay: f64) -> Stat {
    let neg_log_cut = -WEIGHT_CUTOFF.ln();
    let omega = freq * std::f64::consts::TAU;
    let omega2 = omega * omega;
    let dz2_cutoff = if decay > 0.0 {
        neg_log_cut / decay
    } else {
        f64::INFINITY
    };
    let (start, end) = if decay > 0.0 && omega2 > 0.0 {
        let window = (neg_log_cut / (decay * omega2)).sqrt();
        (
            ts.jd.partition_point(|&t| t < tau - window),
            ts.jd.partition_point(|&t| t <= tau + window),
        )
    } else {
        (0, ts.len())
    };

    let mut mat = [[0.0f64; 3]; 3];
    let mut vec = [0.0f64; 3];
    let (mut weight2, mut varw) = (0.0, 0.0);
    for i in start..end {
        let dz = omega * (ts.jd[i] - tau);
        let dz2 = dz * dz;
        if dz2 < dz2_cutoff {
            let w = (-decay * dz2).exp();
            let (cc, ss) = (dz.cos(), dz.sin());
            let (cw, sw) = (w * cc, w * ss);
            mat[0][0] += w;
            weight2 += w * w;
            mat[0][1] += cw;
            mat[0][2] += sw;
            mat[1][1] += cw * cc;
            mat[1][2] += cw * ss;
            mat[2][2] += sw * ss;
            let x = ts.mag[i];
            let xw = w * x;
            vec[0] += xw;
            varw += xw * x;
            vec[1] += cw * x;
            vec[2] += sw * x;
        }
    }

    let mut coef = [0.0f64; 3];
    let neff = if weight2 > 0.0 {
        mat[0][0] * mat[0][0] / weight2
    } else {
        0.0
    };
    let (mut powz, mut amp);
    if neff > 3.0 {
        let m00 = mat[0][0];
        for n1 in 0..3 {
            vec[n1] /= m00;
            for n2 in 1..3 {
                mat[n1][n2] /= m00;
            }
        }
        varw = if m00 > 0.0 { varw / m00 } else { 0.0 };
        mat[0][0] = 1.0;
        let avew = vec[0];
        varw -= avew * avew;
        if varw <= 0.0 {
            varw = 1.0e-12;
        }
        for n1 in 1..3 {
            for n2 in 0..n1 {
                mat[n1][n2] = mat[n2][n1];
            }
        }
        invert3(&mut mat);
        let mut power = 0.0;
        for n1 in 0..3 {
            for n2 in 0..3 {
                coef[n1] += mat[n1][n2] * vec[n2];
            }
            power += coef[n1] * vec[n1];
        }
        power -= avew * avew;
        powz = (neff - 3.0) * power / (varw - power) / 2.0;
        amp = (coef[1] * coef[1] + coef[2] * coef[2]).sqrt();
    } else {
        powz = 0.0;
        amp = 0.0;
    }
    if amp < 1.0e-9 {
        amp = 0.0;
    }
    if powz < 1.0e-9 {
        powz = 0.0;
    }
    Stat {
        tau,
        frequency: freq,
        wwz: powz,
        semi_amplitude: amp,
        mean_mag: coef[0],
        neff: if neff < 1.0e-9 { 0.0 } else { neff },
    }
}

/// Run the transform. `progress` receives the fraction done and returns
/// `false` to cancel.
pub fn transform(
    ts: &TimeSeries,
    params: Params,
    progress: &(dyn Fn(f64) -> bool + Sync),
) -> Result<Transform> {
    validate(&params, ts)?;
    let taus = make_taus(ts, params.time_divisions);
    let frequencies = make_frequencies(params.range);
    if taus.len().saturating_mul(frequencies.len()) > MAX_CELLS {
        return Err(Error::Analysis(format!(
            "that would compute {} cells; use a larger step (at most {MAX_CELLS})",
            taus.len() * frequencies.len()
        )));
    }
    let done = std::sync::atomic::AtomicUsize::new(0);
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let rows: Vec<Option<Vec<Stat>>> = taus
        .par_iter()
        .map(|&tau| {
            use std::sync::atomic::Ordering::Relaxed;
            if cancelled.load(Relaxed) {
                return None;
            }
            let row: Vec<Stat> = frequencies
                .iter()
                .map(|&f| cell(ts, tau, f, params.decay))
                .collect();
            let d = done.fetch_add(1, Relaxed) + 1;
            if !progress(d as f64 / taus.len() as f64) {
                cancelled.store(true, Relaxed);
            }
            Some(row)
        })
        .collect();
    if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    let mut stats = Vec::with_capacity(taus.len() * frequencies.len());
    let mut maxima = Vec::with_capacity(taus.len());
    for row in rows.into_iter().flatten() {
        // The first cell wins a tie, as in VStar (strictly greater replaces).
        let mut best = row[0];
        let mut best_wwz = -1.0;
        for s in &row {
            if s.wwz > best_wwz {
                best_wwz = s.wwz;
                best = *s;
            }
        }
        maxima.push(best);
        stats.extend(row);
    }
    Ok(Transform {
        stats,
        maxima,
        taus,
        frequencies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A signal whose period changes from 10 to 20 days halfway through.
    fn changing() -> TimeSeries {
        let jd: Vec<f64> = (0..800).map(|i| f64::from(i) * 0.5).collect();
        let mag = jd
            .iter()
            .map(|&t| {
                let p = if t < 200.0 { 10.0 } else { 20.0 };
                10.0 + (std::f64::consts::TAU * t / p).sin()
            })
            .collect();
        TimeSeries { jd, mag }
    }

    fn params() -> Params {
        Params {
            range: Range::Period {
                min: 5.0,
                max: 30.0,
                step: 0.25,
            },
            decay: 0.005,
            time_divisions: 20.0,
        }
    }

    #[test]
    fn follows_a_changing_period() {
        let t = transform(&changing(), params(), &|_| true).unwrap();
        assert_eq!(t.stats.len(), t.taus.len() * t.frequencies.len());
        let early = t
            .maxima
            .iter()
            .find(|s| s.tau > 50.0 && s.tau < 100.0)
            .unwrap();
        let late = t
            .maxima
            .iter()
            .find(|s| s.tau > 300.0 && s.tau < 350.0)
            .unwrap();
        assert!((early.period() - 10.0).abs() < 0.8, "{early:?}");
        assert!((late.period() - 20.0).abs() < 1.5, "{late:?}");
        assert!((early.semi_amplitude - 1.0).abs() < 0.15, "{early:?}");
    }

    #[test]
    fn works_over_a_frequency_range_and_cancels() {
        let p = Params {
            range: Range::Frequency {
                min: 0.03,
                max: 0.2,
                step: 0.005,
            },
            ..params()
        };
        assert!(transform(&changing(), p, &|_| true).is_ok());
        assert!(matches!(
            transform(&changing(), p, &|_| false),
            Err(Error::Cancelled)
        ));
    }

    #[test]
    fn rejects_bad_input() {
        let mut p = params();
        p.decay = -1.0;
        assert!(transform(&changing(), p, &|_| true).is_err());
        let few = TimeSeries {
            jd: vec![1.0, 2.0],
            mag: vec![1.0, 2.0],
        };
        assert!(transform(&few, params(), &|_| true).is_err());
    }
}
