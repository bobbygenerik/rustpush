// Adapted from OpenBubbles/rustpush cb2361c (2026-10-04).
// Only the estimator/upgrade gate is borrowed. Delay-only bitrate cuts remain
// disabled in this fork until they can be assessed in a real two-endpoint call.
pub const Q13_SLOPE_RISING: f64 = 500.0;

pub fn q13_slope(samples: impl IntoIterator<Item = (u16, u16)>) -> Option<f64> {
    let mut previous = None;
    let mut origin = None;
    let points = samples.into_iter()
        .filter(|(sequence, _)| previous.replace(*sequence) != Some(*sequence))
        .map(|(sequence, q13)| {
            let origin = *origin.get_or_insert(sequence);
            (sequence.wrapping_sub(origin) as i16 as f64 / 1024.0, q13 as f64)
        }).collect::<Vec<_>>();
    if points.len() < 4 { return None }
    let count = points.len() as f64;
    let mean_x = points.iter().map(|(x, _)| x).sum::<f64>() / count;
    let mean_y = points.iter().map(|(_, y)| y).sum::<f64>() / count;
    let (sxy, sxx) = points.iter().fold((0.0, 0.0), |(sxy, sxx), (x, y)| {
        (sxy + (x - mean_x) * (y - mean_y), sxx + (x - mean_x) * (x - mean_x))
    });
    (sxx > 0.0).then(|| sxy / sxx)
}

pub fn q13_allows_upgrade(slope: Option<f64>) -> bool {
    slope.is_some_and(|slope| slope < Q13_SLOPE_RISING)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_high_latency_does_not_look_congested() {
        let slope = q13_slope([(0, 6000), (128, 6000), (256, 6000), (384, 6000)]);
        assert_eq!(slope, Some(0.0));
        assert!(q13_allows_upgrade(slope));
    }

    #[test]
    fn rising_without_loss_blocks_upgrade() {
        let slope = q13_slope([(0, 1000), (128, 1100), (256, 1200), (384, 1300)]);
        assert_eq!(slope, Some(800.0));
        assert!(!q13_allows_upgrade(slope));
    }

    #[test]
    fn falling_queue_allows_upgrade() {
        let slope = q13_slope([(0, 1300), (128, 1200), (256, 1100), (384, 1000)]);
        assert_eq!(slope, Some(-800.0));
        assert!(q13_allows_upgrade(slope));
    }

    #[test]
    fn duplicate_feedback_does_not_fill_a_thin_window() {
        assert_eq!(q13_slope([(0, 10), (0, 10), (128, 20), (128, 20)]), None);
        assert!(!q13_allows_upgrade(None));
    }

    #[test]
    fn repeated_entries_do_not_reweight_a_spike() {
        let base = [(0, 1000), (128, 3000), (256, 1000), (384, 1000)];
        let repeated = [(0, 1000), (128, 3000), (128, 3000), (128, 3000), (256, 1000), (384, 1000)];
        assert_eq!(q13_slope(base), q13_slope(repeated));
        assert!(q13_allows_upgrade(q13_slope(repeated)));
    }

    #[test]
    fn feedback_clock_wrap_is_a_small_positive_step() {
        assert_eq!(q13_slope([(65408, 1000), (0, 1100), (128, 1200), (256, 1300)]), Some(800.0));
    }

    #[test]
    fn threshold_boundary_is_not_clean() {
        assert!(!q13_allows_upgrade(Some(500.0)));
        assert!(q13_allows_upgrade(Some(499.0)));
    }
}
