//! Checks the Rust DCDFT against numbers produced by VStar's own Java
//! `TSDcDft` for the same data and frequencies.

use vstar_core::analysis::{TimeSeries, dcdft};

#[test]
fn matches_vstar_java_output() {
    let mut ts = TimeSeries::default();
    for line in include_str!("fixtures/dcdft_reference_data.csv").lines() {
        let (jd, mag) = line.split_once(',').unwrap();
        ts.jd.push(jd.parse().unwrap());
        ts.mag.push(mag.parse().unwrap());
    }
    let params = dcdft::Params {
        lo_freq: 0.0105,
        hi_freq: 0.4,
        resolution: 0.0005,
    };
    let spectrum = dcdft::scan(&ts, params, &|_| true).unwrap();
    assert_eq!(spectrum.len(), 780);

    let mut checked = 0;
    for line in include_str!("fixtures/dcdft_reference_expected.txt")
        .lines()
        .filter(|l| !l.starts_with('#'))
    {
        let v: Vec<f64> = line
            .split_whitespace()
            .map(|x| x.parse().unwrap())
            .collect();
        let i = ((v[0] - params.lo_freq) / params.resolution).round() as usize;
        assert!((spectrum.frequency[i] - v[0]).abs() < 1e-9);
        assert!(
            (spectrum.power[i] - v[1]).abs() < 1e-6 * v[1].max(1.0),
            "power at {}: {} vs {}",
            v[0],
            spectrum.power[i],
            v[1]
        );
        assert!(
            (spectrum.amplitude[i] - v[2]).abs() < 1e-8,
            "amplitude at {}: {} vs {}",
            v[0],
            spectrum.amplitude[i],
            v[2]
        );
        checked += 1;
    }
    assert_eq!(checked, 10);

    // The strongest hit is the 27.4 day signal VStar finds.
    let best = spectrum.top_hits(1)[0];
    assert!(
        (best.period - 27.397).abs() < 0.01,
        "best period {}",
        best.period
    );
}
