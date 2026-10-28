//! The "DFT and Spectral Window" tool, ported from VStar's plug-in of that
//! name: Deeming's DFT, its spectral window, a date-compensated DFT and the
//! multi-harmonic DFT of Andronov.

use rayon::prelude::*;

use super::TimeSeries;
use super::dcdft;
use super::models::least_squares;
use crate::{Error, Result};

pub const MAX_HARMONICS: usize = 25;
pub const MAX_FREQUENCIES: usize = 20_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Deeming (1975).
    Dft,
    /// How the sampling of the data shapes the DFT.
    SpectralWindow,
    /// Ferraz-Mello (1981); the same as the DC DFT tool.
    DcDft,
    /// Andronov (1994), with this many harmonics.
    MultiHarmonic(usize),
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dft => "DFT",
            Self::SpectralWindow => "Spectral Window",
            Self::DcDft => "DC DFT",
            Self::MultiHarmonic(_) => "Multi-harmonic DFT",
        }
    }

    pub fn harmonics(self) -> usize {
        match self {
            Self::MultiHarmonic(h) => h,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Params {
    pub min_frequency: f64,
    pub max_frequency: f64,
    pub resolution: f64,
    pub kind: Kind,
}

impl Params {
    /// VStar's "recommended resolution" for a data set of this time span.
    pub fn recommended_resolution(span: f64, harmonics: usize) -> Option<f64> {
        (span > 0.0).then(|| 0.05 / span / harmonics.max(1) as f64)
    }

    pub fn count(&self) -> usize {
        (((self.max_frequency - self.min_frequency) / self.resolution).ceil() as usize) + 1
    }

    pub fn validated(self) -> Result<Self> {
        let bad = |m: &str| Err(Error::Analysis(m.into()));
        if !(self.min_frequency.is_finite() && self.min_frequency >= 0.0) {
            return bad("the minimum frequency can't be negative");
        }
        if !(self.max_frequency.is_finite() && self.max_frequency > self.min_frequency) {
            return bad("the minimum frequency must be less than the maximum frequency");
        }
        if !(self.resolution.is_finite() && self.resolution > 0.0) {
            return bad("the resolution must be greater than zero");
        }
        if let Kind::MultiHarmonic(h) = self.kind
            && !(1..=MAX_HARMONICS).contains(&h)
        {
            return Err(Error::Analysis(format!(
                "the number of harmonics must be between 1 and {MAX_HARMONICS}"
            )));
        }
        if self.count() > MAX_FREQUENCIES {
            return Err(Error::Analysis(format!(
                "that would test {} frequencies; use a coarser resolution (at most {MAX_FREQUENCIES})",
                self.count()
            )));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub frequency: f64,
    pub period: f64,
    pub power: f64,
    pub semi_amplitude: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Spectrum {
    pub points: Vec<Point>,
}

impl Spectrum {
    /// Local maxima of power, strongest first.
    pub fn top_hits(&self, n: usize) -> Vec<Point> {
        let p = &self.points;
        let mut hits: Vec<Point> = (0..p.len())
            .filter(|&i| {
                // As in VStar: a rise into the point and no rise out of it;
                // the first point counts if it beats its neighbour.
                if i == 0 {
                    p.len() > 1 && p[0].power > p[1].power
                } else if i + 1 == p.len() {
                    false
                } else {
                    p[i].power > p[i - 1].power && p[i].power >= p[i + 1].power
                }
            })
            .map(|i| p[i])
            .collect();
        hits.sort_by(|a, b| b.power.total_cmp(&a.power));
        hits.truncate(n);
        hits
    }
}

fn pop_variance(v: &[f64]) -> f64 {
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / v.len() as f64
}

struct Prepared<'a> {
    ts: &'a TimeSeries,
    mean_time: f64,
    mean_mag: f64,
    var_mag: f64,
    zero_cut: f64,
}

impl<'a> Prepared<'a> {
    fn new(ts: &'a TimeSeries) -> Self {
        let n = ts.len() as f64;
        Self {
            ts,
            mean_time: ts.jd.iter().sum::<f64>() / n,
            mean_mag: ts.mag.iter().sum::<f64>() / n,
            var_mag: pop_variance(&ts.mag),
            // From Foster.
            zero_cut: 0.95 / (12.0 * pop_variance(&ts.jd)).sqrt() / 4.0,
        }
    }

    /// `(semi-amplitude, power)` at `nu`.
    fn at(&self, nu: f64, kind: Kind) -> (f64, f64) {
        let ts = self.ts;
        let omega = std::f64::consts::TAU * nu;
        match kind {
            Kind::Dft | Kind::SpectralWindow => {
                let (mut re, mut im) = (0.0, 0.0);
                for (t, m) in ts.jd.iter().zip(&ts.mag) {
                    let a = omega * (t - self.mean_time);
                    let b = if kind == Kind::Dft {
                        m - self.mean_mag
                    } else {
                        0.5
                    };
                    re += b * a.cos();
                    im += b * a.sin();
                }
                let amp = 2.0 * (re * re + im * im).sqrt() / ts.len() as f64;
                (amp, amp * amp)
            }
            Kind::DcDft | Kind::MultiHarmonic(_) => {
                let h = kind.harmonics();
                let cut = if kind == Kind::DcDft {
                    self.zero_cut
                } else {
                    self.zero_cut * h as f64
                };
                if nu < cut {
                    return (f64::NAN, f64::NAN);
                }
                let angles: Vec<f64> = ts.jd.iter().map(|t| omega * (t - self.mean_time)).collect();
                let mut cols: Vec<Vec<f64>> = vec![vec![1.0; ts.len()]];
                for n in 1..=h {
                    cols.push(angles.iter().map(|a| (n as f64 * a).cos()).collect());
                    cols.push(angles.iter().map(|a| (n as f64 * a).sin()).collect());
                }
                let Some(beta) = least_squares(&cols, &ts.mag) else {
                    return (f64::NAN, f64::NAN);
                };
                let predicted: Vec<f64> = (0..ts.len())
                    .map(|i| (1..cols.len()).map(|j| beta[j] * cols[j][i]).sum())
                    .collect();
                let amp = if h == 1 {
                    (beta[1] * beta[1] + beta[2] * beta[2]).sqrt()
                } else {
                    f64::NAN
                };
                let mut power = pop_variance(&predicted) / self.var_mag;
                if kind == Kind::DcDft {
                    power *= (ts.len() - 1) as f64 / 2.0;
                }
                (amp, power)
            }
        }
    }
}

/// Run the chosen transform over the frequency range.
pub fn scan(
    ts: &TimeSeries,
    params: Params,
    progress: &(dyn Fn(f64) -> bool + Sync),
) -> Result<Spectrum> {
    let params = params.validated()?;
    if ts.len() < 4 {
        return Err(Error::Analysis("at least 4 observations are needed".into()));
    }
    let prep = Prepared::new(ts);
    let total = params.count();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let points: Vec<Point> = (0..total)
        .into_par_iter()
        .filter_map(|i| {
            use std::sync::atomic::Ordering::Relaxed;
            if cancelled.load(Relaxed) {
                return None;
            }
            let nu = params.min_frequency + i as f64 * params.resolution;
            let (amp, power) = if nu == 0.0 && params.kind == Kind::DcDft {
                (f64::NAN, f64::NAN)
            } else if params.kind == Kind::DcDft && nu >= prep.zero_cut {
                dcdft::power_at(ts, nu)
                    .map(|p| (p.amplitude, p.power))
                    .unwrap_or((f64::NAN, f64::NAN))
            } else {
                prep.at(nu, params.kind)
            };
            let d = done.fetch_add(1, Relaxed) + 1;
            if d.is_multiple_of(256) && !progress(d as f64 / total as f64) {
                cancelled.store(true, Relaxed);
            }
            Some(Point {
                frequency: nu,
                period: 1.0 / nu,
                power,
                semi_amplitude: amp,
            })
        })
        .collect();
    if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    Ok(Spectrum { points })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(period: f64, amp: f64) -> TimeSeries {
        let jd: Vec<f64> = (0..400)
            .map(|i| f64::from(i) * 0.37 + (f64::from(i) * 7.1).sin())
            .collect();
        let mut jd = jd;
        jd.sort_by(f64::total_cmp);
        let mag = jd
            .iter()
            .map(|t| 10.0 + amp * (std::f64::consts::TAU * t / period).sin())
            .collect();
        TimeSeries { jd, mag }
    }

    fn params(kind: Kind) -> Params {
        Params {
            min_frequency: 0.01,
            max_frequency: 0.5,
            resolution: 0.0005,
            kind,
        }
    }

    #[test]
    fn dft_recovers_period_and_amplitude() {
        let ts = sine(12.0, 0.4);
        let s = scan(&ts, params(Kind::Dft), &|_| true).unwrap();
        let best = s.top_hits(1)[0];
        assert!((best.period - 12.0).abs() < 0.3, "{best:?}");
        assert!((best.semi_amplitude - 0.4).abs() < 0.03, "{best:?}");
    }

    #[test]
    fn spectral_window_peaks_at_zero_frequency() {
        let ts = sine(12.0, 0.4);
        let s = scan(
            &ts,
            Params {
                min_frequency: 0.0,
                ..params(Kind::SpectralWindow)
            },
            &|_| true,
        )
        .unwrap();
        // At zero frequency the window is 1 (all observations in phase).
        assert!((s.points[0].power - 1.0).abs() < 1e-9);
    }

    #[test]
    fn multiharmonic_finds_a_non_sinusoidal_period() {
        let mut ts = sine(12.0, 0.0);
        ts.mag = ts
            .jd
            .iter()
            .map(|t| {
                let x = std::f64::consts::TAU * t / 12.0;
                10.0 + x.sin() + 0.6 * (2.0 * x).cos()
            })
            .collect();
        let s = scan(&ts, params(Kind::MultiHarmonic(2)), &|_| true).unwrap();
        let best = s.top_hits(1)[0];
        assert!((best.period - 12.0).abs() < 0.3, "{best:?}");
    }

    #[test]
    fn dcdft_agrees_with_the_dedicated_tool() {
        let ts = sine(12.0, 0.4);
        let s = scan(&ts, params(Kind::DcDft), &|_| true).unwrap();
        let p = s.points[200];
        let direct = dcdft::power_at(&ts, p.frequency).unwrap();
        assert!((p.power - direct.power).abs() < 1e-9);
    }

    #[test]
    fn rejects_bad_ranges() {
        let bad = Params {
            min_frequency: 0.5,
            max_frequency: 0.1,
            resolution: 0.01,
            kind: Kind::Dft,
        };
        assert!(bad.validated().is_err());
        assert_eq!(Params::recommended_resolution(100.0, 2), Some(0.00025));
    }
}
