//! Statistics needed by the period and model tools: one-way ANOVA and the
//! F distribution.

/// ln(Gamma(x)) by the Lanczos approximation (x > 0).
fn ln_gamma(x: f64) -> f64 {
    const G: [f64; 6] = [
        76.180_091_729_471_46,
        -86.505_320_329_416_77,
        24.014_098_240_830_91,
        -1.231_739_572_450_155,
        0.120_865_097_386_617_9e-2,
        -0.539_523_938_495_3e-5,
    ];
    let mut y = x;
    let tmp = x + 5.5;
    let tmp = tmp - (x + 0.5) * tmp.ln();
    let mut ser = 1.000_000_000_190_015;
    for g in G {
        y += 1.0;
        ser += g / y;
    }
    -tmp + (2.506_628_274_631_000_5 * ser / x).ln()
}

/// Continued fraction for the incomplete beta function.
fn beta_cf(a: f64, b: f64, x: f64) -> f64 {
    const TINY: f64 = 1e-300;
    let (qab, qap, qam) = (a + b, a + 1.0, a - 1.0);
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < TINY {
        d = TINY;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..=1000 {
        let m = f64::from(m);
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-14 {
            break;
        }
    }
    h
}

/// The regularized incomplete beta function I_x(a, b).
pub fn inc_beta(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let bt = (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln()).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        bt * beta_cf(a, b, x) / a
    } else {
        1.0 - bt * beta_cf(b, a, 1.0 - x) / b
    }
}

/// P(F > f) for an F distribution with `d1` and `d2` degrees of freedom.
pub fn f_upper_tail(f: f64, d1: f64, d2: f64) -> f64 {
    if f.is_nan() || f < 0.0 {
        return f64::NAN;
    }
    inc_beta(d2 / 2.0, d1 / 2.0, d2 / (d2 + d1 * f))
}

/// One-way analysis of variance of `groups`. Returns `(F, p)`, or `None`
/// when there are fewer than two groups or no residual degrees of freedom.
/// As in VStar, a group of one value is treated as that value twice.
pub fn one_way_anova(groups: &[Vec<f64>]) -> Option<(f64, f64)> {
    let k = groups.len();
    if k < 2 {
        return None;
    }
    let doubled: Vec<[f64; 2]>;
    let owned: Vec<&[f64]> = if groups.iter().any(|g| g.len() == 1) {
        doubled = groups.iter().map(|g| [g[0], g[0]]).collect();
        groups
            .iter()
            .zip(&doubled)
            .map(|(g, d)| if g.len() == 1 { &d[..] } else { &g[..] })
            .collect()
    } else {
        groups.iter().map(|g| &g[..]).collect()
    };
    let n: usize = owned.iter().map(|g| g.len()).sum();
    let total: f64 = owned.iter().flat_map(|g| g.iter()).sum();
    let grand = total / n as f64;
    let (mut ssb, mut ssw) = (0.0, 0.0);
    for g in &owned {
        let mean = g.iter().sum::<f64>() / g.len() as f64;
        ssb += g.len() as f64 * (mean - grand).powi(2);
        ssw += g.iter().map(|v| (v - mean).powi(2)).sum::<f64>();
    }
    let (dfb, dfw) = ((k - 1) as f64, (n - k) as f64);
    if dfw <= 0.0 {
        return None;
    }
    let f = (ssb / dfb) / (ssw / dfw);
    Some((f, f_upper_tail(f, dfb, dfw)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f_tail_matches_known_values() {
        // F(3, 10) at 3.7083 has upper tail 0.05; F(1, 20) at 4.3512 has 0.05.
        assert!((f_upper_tail(3.7083, 3.0, 10.0) - 0.05).abs() < 1e-4);
        assert!((f_upper_tail(4.3512, 1.0, 20.0) - 0.05).abs() < 1e-4);
        assert!((f_upper_tail(0.0, 4.0, 9.0) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn anova_of_textbook_groups() {
        // Classic example: F = 9.2647, p ~ 0.0043 (see e.g. Wikipedia-style data).
        let groups = vec![
            vec![6.0, 8.0, 4.0, 5.0, 3.0, 4.0],
            vec![8.0, 12.0, 9.0, 11.0, 6.0, 8.0],
            vec![13.0, 9.0, 11.0, 8.0, 7.0, 12.0],
        ];
        let (f, p) = one_way_anova(&groups).unwrap();
        assert!((f - 9.264705882).abs() < 1e-6, "{f}");
        assert!((p - 0.002399).abs() < 5e-4, "{p}");
        assert!(one_way_anova(&groups[..1]).is_none());
    }
}
