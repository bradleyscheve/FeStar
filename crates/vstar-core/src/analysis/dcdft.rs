//! The date-compensated discrete Fourier transform (Ferraz-Mello 1981,
//! Foster 1995).
//!
//! For each trial frequency a constant plus one sine wave is fitted to the
//! magnitudes by least squares, so uneven sampling is handled properly
//! (unlike a plain FFT). The power of a frequency is how much of the
//! variance that sine wave explains. This reproduces VStar's `TSDcDft` with
//! no polynomial trend and no per-observer bias terms.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use rayon::prelude::*;

use super::TimeSeries;
use crate::{Error, Result};

/// Most frequencies a single scan may test. Beyond this the request is
/// rejected rather than left to run for hours.
pub const MAX_FREQUENCIES: usize = 20_000_000;

/// VStar keeps the 100 strongest peaks.
pub const MAX_TOP_HITS: usize = 100;

const FREQUENCIES_PER_BLOCK: usize = 64;

/// What range of frequencies to scan, in cycles per day.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Params {
    pub lo_freq: f64,
    pub hi_freq: f64,
    pub resolution: f64,
}

impl Params {
    /// VStar's "standard scan": a step of `dang0` up to N × `dang0`.
    pub fn standard(ts: &TimeSeries) -> Self {
        let dang0 = base_step(ts);
        Self {
            lo_freq: dang0,
            hi_freq: ts.len() as f64 * dang0,
            resolution: dang0,
        }
    }

    /// A scan over a range of periods (in days). `oversample` is the number
    /// of frequency steps per natural peak width 1/T, where T is the time
    /// span of the data; higher values trace peaks more smoothly.
    pub fn from_period_range(
        ts: &TimeSeries,
        min_period: f64,
        max_period: f64,
        oversample: f64,
    ) -> Result<Self> {
        if !(min_period.is_finite()
            && max_period.is_finite()
            && min_period > 0.0
            && max_period > min_period)
        {
            return Err(Error::Parse(
                "the period range must be positive, with the shortest period first".into(),
            ));
        }
        let span = ts.span();
        if span <= 0.0 {
            return Err(Error::Parse(
                "the data must cover more than one point in time".into(),
            ));
        }
        let params = Self {
            lo_freq: 1.0 / max_period,
            hi_freq: 1.0 / min_period,
            resolution: 1.0 / (oversample.clamp(1.0, 200.0) * span),
        };
        params.validated()
    }

    /// Number of frequencies the scan will test.
    pub fn count(&self) -> usize {
        if self.hi_freq > self.lo_freq && self.resolution > 0.0 {
            ((self.hi_freq - self.lo_freq) / self.resolution) as usize + 1
        } else {
            1
        }
    }

    pub fn validated(self) -> Result<Self> {
        let finite =
            self.lo_freq.is_finite() && self.hi_freq.is_finite() && self.resolution.is_finite();
        if !finite || self.lo_freq < 0.0 || self.hi_freq < self.lo_freq || self.resolution <= 0.0 {
            return Err(Error::Parse(
                "the frequency range and resolution are not valid".into(),
            ));
        }
        if self.count() > MAX_FREQUENCIES {
            return Err(Error::Parse(format!(
                "that scan would test {} frequencies; narrow the period range or lower the resolution",
                self.count()
            )));
        }
        Ok(self)
    }
}

/// VStar's `dang0`: a quarter of the natural frequency resolution, from the
/// variance of the observation times.
fn base_step(ts: &TimeSeries) -> f64 {
    let n = ts.len() as f64;
    if n < 2.0 {
        return 1.0;
    }
    let mean = ts.jd.iter().sum::<f64>() / n;
    let var = ts.jd.iter().map(|t| (t - mean).powi(2)).sum::<f64>() / n;
    if var <= 0.0 {
        return 1.0;
    }
    1.0 / (12.0 * var).sqrt() / 4.0
}

/// One frequency's result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Peak {
    pub frequency: f64,
    pub period: f64,
    pub power: f64,
    /// Semi-amplitude of the fitted sine wave, in magnitudes.
    pub amplitude: f64,
}

/// The result of a scan, one entry per frequency tested.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Spectrum {
    pub frequency: Vec<f64>,
    pub power: Vec<f64>,
    pub amplitude: Vec<f64>,
}

impl Spectrum {
    pub fn len(&self) -> usize {
        self.frequency.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frequency.is_empty()
    }

    fn peak_at(&self, i: usize) -> Peak {
        let f = self.frequency[i];
        Peak {
            frequency: f,
            period: if f > 0.0 { 1.0 / f } else { f64::INFINITY },
            power: self.power[i],
            amplitude: self.amplitude[i],
        }
    }

    /// The strongest local maxima, most powerful first (at most `n`).
    pub fn top_hits(&self, n: usize) -> Vec<Peak> {
        let p = &self.power;
        let mut hits: Vec<Peak> = (1..p.len().saturating_sub(1))
            .filter(|&i| p[i] > 0.0 && p[i] > p[i - 1] && p[i] >= p[i + 1])
            .map(|i| self.peak_at(i))
            .collect();
        hits.sort_by(|a, b| b.power.total_cmp(&a.power));
        hits.truncate(n.min(MAX_TOP_HITS));
        hits
    }

    /// The single strongest frequency, even if it is at the edge of the range.
    pub fn strongest(&self) -> Option<Peak> {
        let i = self
            .power
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))?
            .0;
        Some(self.peak_at(i))
    }
}

/// Summary sums for fitting at one frequency.
struct Fit {
    power: f64,
    amplitude: f64,
}

/// Precomputed values that do not depend on frequency.
struct Prepared {
    /// Times relative to their mean, which keeps phases accurate.
    t: Vec<f64>,
    x: Vec<f64>,
    mean_x: f64,
    var_x: f64,
    n: f64,
    /// Frequencies below this give a power of zero, as in VStar.
    cutoff: f64,
}

impl Prepared {
    fn new(ts: &TimeSeries) -> Result<Self> {
        let n = ts.len();
        if n < 4 {
            return Err(Error::Parse(
                "at least 4 observations are needed for a period search".into(),
            ));
        }
        let nf = n as f64;
        let t_mean = ts.jd.iter().sum::<f64>() / nf;
        let mean_x = ts.mag.iter().sum::<f64>() / nf;
        let var_x = (ts.mag.iter().map(|x| x * x).sum::<f64>() / nf - mean_x * mean_x).max(0.0);
        if var_x <= 0.0 {
            return Err(Error::Parse(
                "every magnitude is identical, so there is no variation to search".into(),
            ));
        }
        Ok(Self {
            t: ts.jd.iter().map(|t| t - t_mean).collect(),
            x: ts.mag.clone(),
            mean_x,
            var_x,
            n: nf,
            cutoff: 0.95 * base_step(ts),
        })
    }

    fn fit(&self, freq: f64) -> Fit {
        if freq < self.cutoff {
            return Fit {
                power: 0.0,
                amplitude: 0.0,
            };
        }
        let w = std::f64::consts::TAU * freq;
        let (mut sc, mut ss, mut scc, mut sss, mut scs, mut sxc, mut sxs) =
            (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        for (&t, &x) in self.t.iter().zip(&self.x) {
            let (s, c) = (w * t).sin_cos();
            sc += c;
            ss += s;
            scc += c * c;
            sss += s * s;
            scs += c * s;
            sxc += x * c;
            sxs += x * s;
        }
        let n = self.n;
        let (sc, ss, scc, sss, scs, sxc, sxs) =
            (sc / n, ss / n, scc / n, sss / n, scs / n, sxc / n, sxs / n);

        // Solve the 3x3 normal equations for (constant, cos, sin).
        let m = [[1.0, sc, ss], [sc, scc, scs], [ss, scs, sss]];
        let v = [self.mean_x, sxc, sxs];
        let Some(coef) = solve3(m, v) else {
            return Fit {
                power: 0.0,
                amplitude: 0.0,
            };
        };
        let explained =
            (coef[0] * v[0] + coef[1] * v[1] + coef[2] * v[2] - self.mean_x * self.mean_x).max(0.0);
        Fit {
            power: (self.n - 1.0) * explained / self.var_x / 2.0,
            amplitude: (coef[1] * coef[1] + coef[2] * coef[2]).sqrt(),
        }
    }
}

/// Solve a 3x3 linear system by Gaussian elimination with pivoting.
fn solve3(mut m: [[f64; 3]; 3], mut v: [f64; 3]) -> Option<[f64; 3]> {
    for col in 0..3 {
        let pivot = (col..3).max_by(|&a, &b| m[a][col].abs().total_cmp(&m[b][col].abs()))?;
        if m[pivot][col].abs() < 1e-12 {
            return None;
        }
        m.swap(col, pivot);
        v.swap(col, pivot);
        for row in col + 1..3 {
            let f = m[row][col] / m[col][col];
            let pivot_row = m[col];
            for (cell, p) in m[row].iter_mut().zip(pivot_row).skip(col) {
                *cell -= f * p;
            }
            v[row] -= f * v[col];
        }
    }
    let mut x = [0.0; 3];
    for row in (0..3).rev() {
        let tail: f64 = (row + 1..3).map(|k| m[row][k] * x[k]).sum();
        x[row] = (v[row] - tail) / m[row][row];
    }
    Some(x)
}

/// Run the scan on all CPU cores. `progress` receives the fraction done
/// (0 to 1) from worker threads and returns `false` to cancel.
pub fn scan(
    ts: &TimeSeries,
    params: Params,
    progress: &(dyn Fn(f64) -> bool + Sync),
) -> Result<Spectrum> {
    let params = params.validated()?;
    let prep = Prepared::new(ts)?;
    let count = params.count();
    let freq_at = |i: usize| params.lo_freq + i as f64 * params.resolution;

    let blocks = count.div_ceil(FREQUENCIES_PER_BLOCK);
    let cancelled = AtomicBool::new(false);
    let done = AtomicUsize::new(0);

    let results: Vec<Vec<(f64, f64)>> = (0..blocks)
        .into_par_iter()
        .map(|b| {
            if cancelled.load(Ordering::Relaxed) {
                return Vec::new();
            }
            let start = b * FREQUENCIES_PER_BLOCK;
            let end = (start + FREQUENCIES_PER_BLOCK).min(count);
            let out: Vec<(f64, f64)> = (start..end)
                .map(|i| {
                    let f = prep.fit(freq_at(i));
                    (f.power, f.amplitude)
                })
                .collect();
            let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
            if !progress(finished as f64 / blocks as f64) {
                cancelled.store(true, Ordering::Relaxed);
            }
            out
        })
        .collect();

    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    let mut spectrum = Spectrum::default();
    for (i, (power, amplitude)) in results.into_iter().flatten().enumerate() {
        spectrum.frequency.push(freq_at(i));
        spectrum.power.push(power);
        spectrum.amplitude.push(amplitude);
    }
    Ok(spectrum)
}

/// Power and amplitude at one frequency.
pub fn power_at(ts: &TimeSeries, frequency: f64) -> Result<Peak> {
    let prep = Prepared::new(ts)?;
    let fit = prep.fit(frequency);
    Ok(Peak {
        frequency,
        period: if frequency > 0.0 {
            1.0 / frequency
        } else {
            f64::INFINITY
        },
        power: fit.power,
        amplitude: fit.amplitude,
    })
}

/// Sharpen a peak found on a coarse grid by searching one grid step either
/// side of it (golden-section search).
pub fn refine_peak(ts: &TimeSeries, peak: Peak, resolution: f64) -> Result<Peak> {
    let prep = Prepared::new(ts)?;
    let (mut lo, mut hi) = (
        (peak.frequency - resolution).max(f64::MIN_POSITIVE),
        peak.frequency + resolution,
    );
    let phi = (5f64.sqrt() - 1.0) / 2.0;
    let (mut a, mut b) = (hi - phi * (hi - lo), lo + phi * (hi - lo));
    let (mut fa, mut fb) = (prep.fit(a).power, prep.fit(b).power);
    for _ in 0..40 {
        if fa < fb {
            lo = a;
            a = b;
            fa = fb;
            b = lo + phi * (hi - lo);
            fb = prep.fit(b).power;
        } else {
            hi = b;
            b = a;
            fb = fa;
            a = hi - phi * (hi - lo);
            fa = prep.fit(a).power;
        }
    }
    let f = (lo + hi) / 2.0;
    let fit = prep.fit(f);
    // Never return something worse than what we started with.
    if fit.power < peak.power {
        return Ok(peak);
    }
    Ok(Peak {
        frequency: f,
        period: 1.0 / f,
        power: fit.power,
        amplitude: fit.amplitude,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sinusoid of the given period sampled at uneven times.
    fn sine_series(period: f64, n: usize, amplitude: f64) -> TimeSeries {
        let jd: Vec<f64> = (0..n)
            .map(|i| 2_460_000.0 + i as f64 * 1.37 + ((i * 7919) % 100) as f64 * 0.003)
            .collect();
        let mag = jd
            .iter()
            .map(|t| 10.0 + amplitude * (std::f64::consts::TAU * (t - 2_460_000.0) / period).sin())
            .collect();
        TimeSeries { jd, mag }
    }

    #[test]
    fn finds_the_period_of_a_sine_wave() {
        let ts = sine_series(33.3, 300, 0.5);
        let params = Params::from_period_range(&ts, 2.0, 200.0, 20.0).unwrap();
        let spectrum = scan(&ts, params, &|_| true).unwrap();
        let best = spectrum.top_hits(5)[0];
        assert!((best.period - 33.3).abs() < 0.2, "found {}", best.period);
        let refined = refine_peak(&ts, best, params.resolution).unwrap();
        assert!(
            (refined.period - 33.3).abs() < 0.01,
            "refined to {}",
            refined.period
        );
        assert!(
            (refined.amplitude - 0.5).abs() < 0.01,
            "amplitude {}",
            refined.amplitude
        );
    }

    #[test]
    fn power_matches_an_independent_least_squares_fit() {
        let mut ts = sine_series(12.0, 80, 0.3);
        for (i, m) in ts.mag.iter_mut().enumerate() {
            *m += 0.05 * ((i * 31 % 17) as f64 - 8.0) / 8.0;
        }
        let freq = 1.0 / 12.5;
        let got = power_at(&ts, freq).unwrap();

        // Independent route: fit by solving the normal equations in f64 with
        // explicit sums, then use power = (N-1) * R^2 / 2.
        let n = ts.len() as f64;
        let t0 = ts.jd.iter().sum::<f64>() / n;
        let w = std::f64::consts::TAU * freq;
        let basis: Vec<[f64; 3]> = ts
            .jd
            .iter()
            .map(|t| [1.0, (w * (t - t0)).cos(), (w * (t - t0)).sin()])
            .collect();
        let mut ata = [[0.0; 3]; 3];
        let mut atb = [0.0; 3];
        for (b, y) in basis.iter().zip(&ts.mag) {
            for i in 0..3 {
                atb[i] += b[i] * y;
                for j in 0..3 {
                    ata[i][j] += b[i] * b[j];
                }
            }
        }
        let c = solve3(ata, atb).unwrap();
        let mean = ts.mag.iter().sum::<f64>() / n;
        let sst: f64 = ts.mag.iter().map(|y| (y - mean).powi(2)).sum();
        let sse: f64 = basis
            .iter()
            .zip(&ts.mag)
            .map(|(b, y)| (y - (c[0] * b[0] + c[1] * b[1] + c[2] * b[2])).powi(2))
            .sum();
        let expected = (n - 1.0) * (1.0 - sse / sst) / 2.0;
        assert!(
            (got.power - expected).abs() < 1e-6 * expected.max(1.0),
            "{} vs {}",
            got.power,
            expected
        );
        assert!((got.amplitude - (c[1] * c[1] + c[2] * c[2]).sqrt()).abs() < 1e-9);
    }

    #[test]
    fn standard_scan_follows_vstar_defaults() {
        let ts = sine_series(10.0, 100, 0.2);
        let p = Params::standard(&ts);
        assert_eq!(p.count(), 100);
        assert!((p.hi_freq - 100.0 * p.resolution).abs() < 1e-12);
        let spectrum = scan(&ts, p, &|_| true).unwrap();
        assert_eq!(spectrum.len(), 100);
    }

    #[test]
    fn rejects_bad_input() {
        let ts = sine_series(10.0, 50, 0.2);
        assert!(Params::from_period_range(&ts, 5.0, 2.0, 10.0).is_err());
        assert!(Params::from_period_range(&ts, -1.0, 2.0, 10.0).is_err());
        let huge = Params {
            lo_freq: 0.0,
            hi_freq: 1e9,
            resolution: 1e-3,
        };
        assert!(huge.validated().is_err());
        let tiny = TimeSeries {
            jd: vec![1.0, 2.0],
            mag: vec![1.0, 2.0],
        };
        assert!(
            scan(
                &tiny,
                Params {
                    lo_freq: 0.1,
                    hi_freq: 0.2,
                    resolution: 0.01
                },
                &|_| true
            )
            .is_err()
        );
        let flat = TimeSeries {
            jd: vec![1.0, 2.0, 3.0, 4.0, 5.0],
            mag: vec![9.0; 5],
        };
        assert!(
            scan(
                &flat,
                Params {
                    lo_freq: 0.1,
                    hi_freq: 0.2,
                    resolution: 0.01
                },
                &|_| true
            )
            .is_err()
        );
    }

    #[test]
    fn can_be_cancelled() {
        let ts = sine_series(10.0, 200, 0.2);
        let params = Params::from_period_range(&ts, 1.0, 100.0, 50.0).unwrap();
        assert!(matches!(
            scan(&ts, params, &|_| false),
            Err(Error::Cancelled)
        ));
    }
}
