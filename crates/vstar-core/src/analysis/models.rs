//! Models fitted to a light curve: polynomial, Fourier (several periods and
//! their harmonics), Loess smoothing and a piecewise linear model through
//! binned means. Ported from VStar's model creators.

use super::TimeSeries;
use super::binning::symmetric_bins;
use crate::{Error, Result};

/// A fitted model: its value at each observation, the residuals, quality
/// measures and a smooth curve for drawing.
#[derive(Debug, Clone)]
pub struct ModelFit {
    pub kind: String,
    pub description: String,
    /// Times of the observations used.
    pub times: Vec<f64>,
    pub fit: Vec<f64>,
    pub residuals: Vec<f64>,
    pub rms: f64,
    pub aic: f64,
    pub bic: f64,
    /// Points of the model for drawing, sorted by time.
    pub curve: Vec<[f64; 2]>,
    /// Extra details worth showing, one per line.
    pub notes: Vec<String>,
}

impl ModelFit {
    fn new(
        kind: &str,
        description: String,
        ts: &TimeSeries,
        fit: Vec<f64>,
        parameters: f64,
        curve: Vec<[f64; 2]>,
        notes: Vec<String>,
    ) -> Self {
        let residuals: Vec<f64> = ts.mag.iter().zip(&fit).map(|(m, f)| m - f).collect();
        let n = residuals.len() as f64;
        let sum_sq: f64 = residuals.iter().map(|r| r * r).sum();
        let rms = (sum_sq / n).sqrt();
        let (aic, bic) = if sum_sq / n > 0.0 {
            let common = n * (sum_sq / n).ln();
            (common + 2.0 * parameters, common + parameters * n.ln())
        } else {
            (f64::NAN, f64::NAN)
        };
        Self {
            kind: kind.into(),
            description,
            times: ts.jd.clone(),
            fit,
            residuals,
            rms,
            aic,
            bic,
            curve,
            notes,
        }
    }
}

/// Evenly spaced times across the data, for drawing a model.
fn grid(ts: &TimeSeries, points: usize) -> Vec<f64> {
    let (a, b) = (ts.jd[0], ts.jd[ts.len() - 1]);
    (0..points)
        .map(|i| a + (b - a) * i as f64 / (points - 1) as f64)
        .collect()
}

fn require(ts: &TimeSeries, n: usize, what: &str) -> Result<()> {
    if ts.len() < n {
        return Err(Error::Analysis(format!(
            "at least {n} observations are needed for {what}"
        )));
    }
    Ok(())
}

/// Least squares by Householder QR. `cols` holds the columns of the design
/// matrix. Returns `None` if the columns are linearly dependent.
pub(crate) fn least_squares(cols: &[Vec<f64>], y: &[f64]) -> Option<Vec<f64>> {
    let (m, n) = (y.len(), cols.len());
    if m < n || n == 0 {
        return None;
    }
    let mut a: Vec<Vec<f64>> = cols.to_vec();
    let mut b = y.to_vec();
    for k in 0..n {
        let norm = a[k][k..].iter().map(|v| v * v).sum::<f64>().sqrt();
        if norm < 1e-12 {
            return None;
        }
        let alpha = if a[k][k] > 0.0 { -norm } else { norm };
        let mut v: Vec<f64> = a[k][k..].to_vec();
        v[0] -= alpha;
        let vnorm2: f64 = v.iter().map(|x| x * x).sum();
        if vnorm2 == 0.0 {
            continue;
        }
        for col in a.iter_mut().skip(k) {
            let dot: f64 = col[k..].iter().zip(&v).map(|(x, y)| x * y).sum();
            let f = 2.0 * dot / vnorm2;
            for (x, vi) in col[k..].iter_mut().zip(&v) {
                *x -= f * vi;
            }
        }
        let dot: f64 = b[k..].iter().zip(&v).map(|(x, y)| x * y).sum();
        let f = 2.0 * dot / vnorm2;
        for (x, vi) in b[k..].iter_mut().zip(&v) {
            *x -= f * vi;
        }
    }
    let mut x = vec![0.0; n];
    for k in (0..n).rev() {
        let mut s = b[k];
        for j in k + 1..n {
            s -= a[j][k] * x[j];
        }
        let d = a[k][k];
        if d.abs() < 1e-12 {
            return None;
        }
        x[k] = s / d;
    }
    Some(x)
}

// ---------------------------------------------------------------- polynomial

pub const MAX_POLYNOMIAL_DEGREE: usize = 30;

/// Chebyshev polynomials T_0..T_degree at `u` in [-1, 1].
fn chebyshev(u: f64, degree: usize) -> Vec<f64> {
    let mut t = Vec::with_capacity(degree + 1);
    t.push(1.0);
    if degree >= 1 {
        t.push(u);
    }
    for k in 2..=degree {
        t.push(2.0 * u * t[k - 1] - t[k - 2]);
    }
    t
}

/// Fit a polynomial of the given degree (0 to 30) to the light curve.
pub fn polynomial_fit(ts: &TimeSeries, degree: usize) -> Result<ModelFit> {
    if degree > MAX_POLYNOMIAL_DEGREE {
        return Err(Error::Analysis(format!(
            "the polynomial degree can be at most {MAX_POLYNOMIAL_DEGREE}"
        )));
    }
    require(ts, degree + 2, "a polynomial of this degree")?;
    let (a, b) = (ts.jd[0], ts.jd[ts.len() - 1]);
    if b <= a {
        return Err(Error::Analysis(
            "the observations all have the same time".into(),
        ));
    }
    let (mid, half) = ((a + b) / 2.0, (b - a) / 2.0);
    let basis = |t: f64| chebyshev((t - mid) / half, degree);
    let rows: Vec<Vec<f64>> = ts.jd.iter().map(|&t| basis(t)).collect();
    let cols: Vec<Vec<f64>> = (0..=degree)
        .map(|k| rows.iter().map(|r| r[k]).collect())
        .collect();
    let coef = least_squares(&cols, &ts.mag).ok_or_else(|| {
        Error::Analysis("the polynomial can't be fitted (too few distinct times)".into())
    })?;
    let eval = |t: f64| basis(t).iter().zip(&coef).map(|(b, c)| b * c).sum::<f64>();
    let fit: Vec<f64> = ts.jd.iter().map(|&t| eval(t)).collect();
    let curve = grid(ts, 2000).into_iter().map(|t| [t, eval(t)]).collect();
    Ok(ModelFit::new(
        "Polynomial fit",
        format!("Polynomial fit of degree {degree}"),
        ts,
        fit,
        (degree + 1) as f64,
        curve,
        Vec::new(),
    ))
}

// ------------------------------------------------------------------- Fourier

/// A period and how many of its harmonics to include.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PeriodTerm {
    pub period: f64,
    pub harmonics: usize,
}

/// Fit a constant plus a sine and cosine at each period's frequency and at
/// the frequencies of its harmonics, all at once.
pub fn fourier_fit(ts: &TimeSeries, terms: &[PeriodTerm]) -> Result<ModelFit> {
    if terms.is_empty() {
        return Err(Error::Analysis("give at least one period".into()));
    }
    for t in terms {
        if !(t.period.is_finite() && t.period > 0.0) {
            return Err(Error::Analysis("periods must be greater than zero".into()));
        }
        if t.harmonics == 0 || t.harmonics > 20 {
            return Err(Error::Analysis(
                "the number of harmonics must be between 1 and 20".into(),
            ));
        }
    }
    let freqs: Vec<(f64, usize, usize)> = terms
        .iter()
        .enumerate()
        .flat_map(|(i, t)| (1..=t.harmonics).map(move |k| (k as f64 / t.period, i, k)))
        .collect();
    require(ts, 2 * freqs.len() + 2, "this many terms")?;
    let t0 = (ts.jd[0] + ts.jd[ts.len() - 1]) / 2.0;
    let basis = |t: f64| -> Vec<f64> {
        let mut v = vec![1.0];
        for (f, _, _) in &freqs {
            let x = std::f64::consts::TAU * f * (t - t0);
            v.push(x.cos());
            v.push(x.sin());
        }
        v
    };
    let rows: Vec<Vec<f64>> = ts.jd.iter().map(|&t| basis(t)).collect();
    let cols: Vec<Vec<f64>> = (0..rows[0].len())
        .map(|k| rows.iter().map(|r| r[k]).collect())
        .collect();
    let coef = least_squares(&cols, &ts.mag).ok_or_else(|| {
        Error::Analysis(
            "the model can't be fitted; the periods may be too close together or the data too sparse"
                .into(),
        )
    })?;
    let eval = |t: f64| basis(t).iter().zip(&coef).map(|(b, c)| b * c).sum::<f64>();
    let fit: Vec<f64> = ts.jd.iter().map(|&t| eval(t)).collect();

    let shortest = freqs
        .iter()
        .map(|f| 1.0 / f.0)
        .fold(f64::INFINITY, f64::min);
    let points = ((ts.span() / shortest) * 24.0)
        .ceil()
        .clamp(2000.0, 200_000.0) as usize;
    let curve = grid(ts, points).into_iter().map(|t| [t, eval(t)]).collect();

    let mut notes = vec![format!("Constant: {:.5}", coef[0])];
    for (j, (f, i, k)) in freqs.iter().enumerate() {
        let (c, s) = (coef[1 + 2 * j], coef[2 + 2 * j]);
        notes.push(format!(
            "Period {:.5} d (term {}, harmonic {}): amplitude {:.5}",
            1.0 / f,
            i + 1,
            k,
            (c * c + s * s).sqrt()
        ));
    }
    Ok(ModelFit::new(
        "Fourier model",
        format!("Fourier model with {} periods", terms.len()),
        ts,
        fit,
        (2 * freqs.len() + 1) as f64,
        curve,
        notes,
    ))
}

// --------------------------------------------------------------------- Loess

pub const LOESS_BANDWIDTH: f64 = 0.3;
pub const LOESS_ROBUSTNESS_ITERATIONS: usize = 2;
const LOESS_ACCURACY: f64 = 1e-12;

fn next_nonzero(weights: &[f64], i: usize) -> usize {
    let mut j = i + 1;
    while j < weights.len() && weights[j] == 0.0 {
        j += 1;
    }
    j
}

fn tricube(x: f64) -> f64 {
    let t = 1.0 - x * x * x;
    t * t * t
}

/// Local linear regression with tricube weights and robustness iterations,
/// as in Apache Commons Math's `LoessInterpolator`. `x` must be strictly
/// increasing.
pub fn loess_smooth(x: &[f64], y: &[f64], bandwidth: f64, robustness: usize) -> Result<Vec<f64>> {
    let n = x.len();
    if n == 1 {
        return Ok(y.to_vec());
    }
    let bw_points = (bandwidth * n as f64) as usize;
    if bw_points < 2 {
        return Err(Error::Analysis(format!(
            "the bandwidth is too small for {n} observations: it must cover at least 2 points"
        )));
    }
    let weights = vec![1.0; n];
    let mut robust = vec![1.0; n];
    let mut res = vec![0.0; n];
    let mut residuals = vec![0.0; n];
    for iter in 0..=robustness {
        let mut interval = (0usize, bw_points - 1);
        for i in 0..n {
            let xi = x[i];
            if i > 0 {
                let next_right = next_nonzero(&weights, interval.1);
                if next_right < n && x[next_right] - x[i] < x[i] - x[interval.0] {
                    let next_left = next_nonzero(&weights, interval.0);
                    interval = (next_left, next_right);
                }
            }
            let (il, ir) = interval;
            let edge = if x[i] - x[il] > x[ir] - x[i] { il } else { ir };
            let (mut sw, mut sx, mut sxx, mut sy, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
            let denom = (1.0 / (x[edge] - xi)).abs();
            for k in il..=ir {
                let (xk, yk) = (x[k], y[k]);
                let dist = if k < i { xi - xk } else { xk - xi };
                let w = tricube(dist * denom);
                let wght = w * robust[k] * weights[k];
                let xkw = xk * wght;
                sw += wght;
                sx += xkw;
                sxx += xk * xkw;
                sy += yk * wght;
                sxy += yk * xkw;
            }
            let (mx, my, mxy, mxx) = (sx / sw, sy / sw, sxy / sw, sxx / sw);
            let beta = if (mxx - mx * mx).abs().sqrt() < LOESS_ACCURACY {
                0.0
            } else {
                (mxy - mx * my) / (mxx - mx * mx)
            };
            let alpha = my - beta * mx;
            res[i] = beta * xi + alpha;
            residuals[i] = (y[i] - res[i]).abs();
        }
        if iter == robustness {
            break;
        }
        let mut sorted = residuals.clone();
        sorted.sort_by(f64::total_cmp);
        let median = sorted[n / 2];
        if median < LOESS_ACCURACY {
            break;
        }
        for i in 0..n {
            let arg = residuals[i] / (6.0 * median);
            robust[i] = if arg >= 1.0 {
                0.0
            } else {
                let s = 1.0 - arg * arg;
                s * s
            };
        }
    }
    Ok(res)
}

/// A natural cubic spline through `(x, y)`.
struct Spline {
    x: Vec<f64>,
    y: Vec<f64>,
    /// Second derivatives at the knots.
    m: Vec<f64>,
}

impl Spline {
    fn natural(x: &[f64], y: &[f64]) -> Self {
        let n = x.len();
        let mut m = vec![0.0; n];
        if n > 2 {
            // Tridiagonal solve for the interior second derivatives.
            let mut c = vec![0.0; n];
            let mut d = vec![0.0; n];
            for i in 1..n - 1 {
                let h0 = x[i] - x[i - 1];
                let h1 = x[i + 1] - x[i];
                let diag = 2.0 * (h0 + h1);
                let rhs = 6.0 * ((y[i + 1] - y[i]) / h1 - (y[i] - y[i - 1]) / h0);
                let denom = diag - h0 * c[i - 1];
                c[i] = h1 / denom;
                d[i] = (rhs - h0 * d[i - 1]) / denom;
            }
            for i in (1..n - 1).rev() {
                m[i] = d[i] - c[i] * m[i + 1];
            }
        }
        Self {
            x: x.to_vec(),
            y: y.to_vec(),
            m,
        }
    }

    fn value(&self, t: f64) -> f64 {
        let n = self.x.len();
        if n == 1 {
            return self.y[0];
        }
        let i = match self.x.partition_point(|&k| k <= t) {
            0 => 0,
            p if p >= n => n - 2,
            p => p - 1,
        };
        let h = self.x[i + 1] - self.x[i];
        let (a, b) = ((self.x[i + 1] - t) / h, (t - self.x[i]) / h);
        a * self.y[i]
            + b * self.y[i + 1]
            + ((a * a * a - a) * self.m[i] + (b * b * b - b) * self.m[i + 1]) * h * h / 6.0
    }
}

/// The observations with repeated times collapsed to one (the last wins),
/// as VStar does before Loess smoothing.
fn distinct_times(ts: &TimeSeries) -> (Vec<f64>, Vec<f64>) {
    let (mut x, mut y): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
    for (&t, &m) in ts.jd.iter().zip(&ts.mag) {
        if x.last() == Some(&t) {
            *y.last_mut().unwrap() = m;
        } else {
            x.push(t);
            y.push(m);
        }
    }
    (x, y)
}

/// Loess smoothing with VStar's defaults (bandwidth 0.3, 2 robustness
/// iterations), joined by a cubic spline.
pub fn loess_fit(ts: &TimeSeries, bandwidth: f64, robustness: usize) -> Result<ModelFit> {
    require(ts, 7, "a Loess fit")?;
    if !(0.0..=1.0).contains(&bandwidth) || bandwidth == 0.0 {
        return Err(Error::Analysis(
            "the bandwidth must be greater than 0 and at most 1".into(),
        ));
    }
    let (x, y) = distinct_times(ts);
    let smooth = loess_smooth(&x, &y, bandwidth, robustness)?;
    let spline = Spline::natural(&x, &smooth);
    let fit: Vec<f64> = ts.jd.iter().map(|&t| spline.value(t)).collect();
    let curve = grid(ts, 2000)
        .into_iter()
        .map(|t| [t, spline.value(t)])
        .collect();
    Ok(ModelFit::new(
        "Loess fit",
        format!("Loess fit (bandwidth {bandwidth}, {robustness} robustness iterations)"),
        ts,
        fit,
        3.0,
        curve,
        Vec::new(),
    ))
}

// ----------------------------------------------------------------- piecewise

/// VStar's default bin size for the means series, in days.
pub const DEFAULT_BIN_DAYS: f64 = 20.0;

/// Join the means of bins `bin_days` wide with straight lines.
pub fn piecewise_linear_fit(ts: &TimeSeries, bin_days: f64) -> Result<ModelFit> {
    require(ts, 4, "a piecewise linear model")?;
    if !(bin_days.is_finite() && bin_days > 0.0) {
        return Err(Error::Analysis(
            "the bin size must be greater than zero".into(),
        ));
    }
    let means: Vec<[f64; 2]> = symmetric_bins(&ts.jd, &ts.mag, bin_days)
        .into_iter()
        .map(|b| [b.mean_time, b.mean_mag])
        .collect();
    if means.len() < 2 {
        return Err(Error::Analysis(
            "the bins are too wide: at least 2 bins are needed".into(),
        ));
    }
    let eval = |t: f64| {
        let i = match means.partition_point(|p| p[0] <= t) {
            0 => 0,
            p if p >= means.len() => means.len() - 2,
            p => p - 1,
        };
        let (a, b) = (means[i], means[i + 1]);
        a[1] + (b[1] - a[1]) * (t - a[0]) / (b[0] - a[0])
    };
    let fit: Vec<f64> = ts.jd.iter().map(|&t| eval(t)).collect();
    let segments = means.len() - 1;
    Ok(ModelFit::new(
        "Piecewise linear model",
        format!("Piecewise linear model with {segments} functions (bins of {bin_days} days)"),
        ts,
        fit,
        segments as f64,
        means,
        Vec::new(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn series(f: impl Fn(f64) -> f64, n: usize, step: f64) -> TimeSeries {
        let jd: Vec<f64> = (0..n).map(|i| 2_450_000.0 + i as f64 * step).collect();
        let mag = jd.iter().map(|&t| f(t)).collect();
        TimeSeries { jd, mag }
    }

    #[test]
    fn polynomial_recovers_a_cubic() {
        let ts = series(
            |t| {
                let u = (t - 2_450_050.0) / 50.0;
                9.0 + 0.5 * u - 0.3 * u * u + 0.1 * u * u * u
            },
            200,
            0.5,
        );
        let m = polynomial_fit(&ts, 3).unwrap();
        assert!(m.rms < 1e-8, "{}", m.rms);
        assert!(polynomial_fit(&ts, 31).is_err());
    }

    #[test]
    fn fourier_recovers_period_and_harmonic() {
        let ts = series(
            |t| {
                let x = std::f64::consts::TAU * (t - 2_450_000.0) / 12.0;
                10.0 + 0.8 * x.cos() + 0.3 * (2.0 * x).sin()
            },
            400,
            0.37,
        );
        let m = fourier_fit(
            &ts,
            &[PeriodTerm {
                period: 12.0,
                harmonics: 2,
            }],
        )
        .unwrap();
        assert!(m.rms < 1e-8, "{}", m.rms);
        assert!(m.notes[1].contains("amplitude 0.8"));
        assert!(m.notes[2].contains("amplitude 0.3"));
    }

    // The test data and expected values come from VStar's own Loess test.
    const DATA: [(f64, f64); 20] = [
        (47551.614000, 9.30710),
        (47560.675000, 9.34120),
        (47568.928000, 9.44000),
        (47580.007200, 9.39380),
        (47599.200100, 9.38330),
        (47609.626200, 9.44620),
        (47630.002000, 9.26000),
        (47638.450000, 9.27500),
        (47650.587000, 8.87000),
        (47662.466700, 8.86670),
        (47671.247800, 8.65000),
        (47680.044400, 8.43330),
        (47691.020000, 8.06000),
        (47698.759600, 8.05560),
        (47710.560700, 8.13570),
        (47719.444600, 8.27500),
        (47730.007800, 8.40710),
        (47741.224600, 8.57220),
        (47749.907000, 8.92000),
        (47761.632300, 9.35620),
    ];
    const EXPECTED: [f64; 20] = [
        9.32427713698,
        9.353292510661,
        9.375505873301,
        9.397557609056,
        9.400853350758,
        9.369149154407,
        9.221411150705,
        9.148846450102,
        8.981614180433,
        8.78140656781,
        8.621607416169,
        8.434780938625,
        8.24424033749,
        8.16158493011,
        8.172503978765,
        8.269947627992,
        8.447879132802,
        8.700017593701,
        8.95298995942,
        9.30568196334,
    ];

    #[test]
    fn loess_matches_vstar() {
        let ts = TimeSeries {
            jd: DATA.iter().map(|d| d.0).collect(),
            mag: DATA.iter().map(|d| d.1).collect(),
        };
        let m = loess_fit(&ts, LOESS_BANDWIDTH, LOESS_ROBUSTNESS_ITERATIONS).unwrap();
        for (got, want) in m.fit.iter().zip(EXPECTED) {
            assert!((got - want).abs() < 1e-6, "{got} vs {want}");
        }
    }

    #[test]
    fn piecewise_passes_through_the_means() {
        let ts = series(|t| ((t - 2_450_000.0) / 30.0).sin(), 300, 1.0);
        let m = piecewise_linear_fit(&ts, 20.0).unwrap();
        assert!(m.curve.len() >= 10);
        // The fit follows the smooth curve closely.
        assert!(m.rms < 0.05, "{}", m.rms);
        assert!(piecewise_linear_fit(&ts, 0.0).is_err());
        // Very wide bins leave one on each side of the centre.
        assert_eq!(piecewise_linear_fit(&ts, 1.0e6).unwrap().curve.len(), 2);
    }
}
