//! Symmetric binning of sorted observations, ported from VStar's
//! `DescStats.createSymmetricBinnedObservations`. Bins grow outward from the
//! middle observation so that they are symmetric about the centre of the data.

/// One bin: the mean time and magnitude, and the magnitudes it holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Bin {
    pub mean_time: f64,
    pub mean_mag: f64,
    pub mags: Vec<f64>,
}

fn make_bin(times: &[f64], mags: &[f64], lo: usize, hi: usize) -> Bin {
    let n = (hi - lo + 1) as f64;
    let mean_time = times[lo..=hi].iter().sum::<f64>() / n;
    let mean_mag = mags[lo..=hi].iter().sum::<f64>() / n;
    Bin {
        mean_time,
        mean_mag,
        mags: mags[lo..=hi].to_vec(),
    }
}

/// Bin `mags` (ordered by `times`, which must be ascending) into bins that
/// span at most `width` time units.
pub fn symmetric_bins(times: &[f64], mags: &[f64], width: f64) -> Vec<Bin> {
    let n = times.len();
    if n < 2 || !(width.is_finite() && width > 0.0) {
        return Vec::new();
    }

    // From the middle observation leftward.
    let mut left: Vec<Bin> = Vec::new();
    let mut max_index = n / 2 - 1;
    let mut max_time = times[max_index];
    let mut i = max_index as isize - 1;
    loop {
        if i >= 0 && times[i as usize] + width > max_time {
            i -= 1;
        } else {
            let bin = make_bin(times, mags, (i + 1) as usize, max_index);
            if !bin.mean_mag.is_nan() {
                left.push(bin);
            }
            if i >= 0 {
                max_index = i as usize;
                max_time = times[max_index];
                i -= 1;
            } else {
                break;
            }
        }
    }
    left.reverse();

    // From the middle observation rightward.
    let mut min_index = n / 2;
    let mut min_time = times[min_index];
    let mut i = min_index + 1;
    loop {
        if i < n && min_time + width > times[i] {
            i += 1;
        } else {
            let bin = make_bin(times, mags, min_index, i - 1);
            if !bin.mean_mag.is_nan() {
                left.push(bin);
            }
            if i < n {
                min_index = i;
                min_time = times[min_index];
                i += 1;
            } else {
                break;
            }
        }
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bins_cover_every_observation_once() {
        let times: Vec<f64> = (0..50).map(|i| f64::from(i) * 0.37).collect();
        let mags: Vec<f64> = times.iter().map(|t| t.sin()).collect();
        let bins = symmetric_bins(&times, &mags, 2.0);
        assert_eq!(bins.iter().map(|b| b.mags.len()).sum::<usize>(), 50);
        assert!(bins.windows(2).all(|w| w[0].mean_time < w[1].mean_time));
    }

    #[test]
    fn too_little_data_gives_no_bins() {
        assert!(symmetric_bins(&[1.0], &[1.0], 1.0).is_empty());
    }
}
