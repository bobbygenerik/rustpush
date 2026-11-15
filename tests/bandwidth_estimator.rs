use rustpush::bandwidth_estimator::{
    bandwidth_estimation_kbps_for_stats, compute_bw_estimation_from_map,
    CollectorBandwidthOutput, ServerBagBwEstimationParams, VCRateControlBandwidthEstimator,
    VCRateControlBandwidthEstimatorMap,
};

#[test]
fn test_init_0x2063b934c() {
    let est = VCRateControlBandwidthEstimator::new();
    // 0x2063b9384..0x2063b9398
    assert_eq!(est.sudden_change_parameters.duration_threshold_upward, 0.5);
    assert_eq!(est.sudden_change_parameters.duration_threshold_downward, 10.0);
    // 0x2063b939c
    assert_eq!(est.min_probing_sequence_size, 250);
    assert_eq!(est.estimated_bandwidth, 0.0);
    assert!(!est.is_initialized);
    assert!(!est.is_first_packet);
}

#[test]
fn test_update_qualification_parameters_rat5_0x2063b9494() {
    let mut est = VCRateControlBandwidthEstimator::new();
    est.set_radio_access_technology(5);
    est.set_mode(1);
    assert_eq!(est.max_detectable_bandwidth, 400_000.0);
    assert_eq!(est.min_estimation_window_duration, 0.002);
    assert_eq!(est.max_over_range_count, 0);
    assert_eq!(est.min_packets_count, 1);

    est.set_mode(3);
    assert_eq!(est.max_detectable_bandwidth, 800_000.0);
}

#[test]
fn test_update_qualification_parameters_other_rat_modes_0x2063b94c8() {
    let mut est = VCRateControlBandwidthEstimator::new();
    est.set_radio_access_technology(3);
    est.set_mode(1);
    assert_eq!(est.max_detectable_bandwidth, 6_500_000.0);
    assert_eq!(est.min_estimation_window_duration, 0.008);
    assert_eq!(est.max_over_range_count, 3);
    assert_eq!(est.min_packets_count, 1);

    est.set_l4s_high_data_rate_enabled(true);
    est.set_fast_sudden_bandwidth_detection_enabled(true);
    assert_eq!(est.max_detectable_bandwidth, 23_000_000.0);
    assert_eq!(est.max_over_range_count, 0);

    est.set_mode(3);
    assert_eq!(est.max_detectable_bandwidth, 5_000_000.0);
    est.set_mode(5);
    assert_eq!(est.max_detectable_bandwidth, 10_000_000.0);
    est.set_mode(8);
    assert_eq!(est.max_detectable_bandwidth, 60_000_000.0);
    est.set_mode(12);
    assert_eq!(est.max_detectable_bandwidth, 60_000_000.0);
    assert_eq!(est.min_estimation_window_duration, 0.002);
    assert_eq!(est.max_over_range_count, 1);
}

#[test]
fn test_update_bwe_non_positive_0x2063b9efa() {
    let mut est = VCRateControlBandwidthEstimator::new();
    assert_eq!(est.update_bandwidth_estimation(0.0, 1.0), 0.0);
    assert_eq!(est.sudden_change_state, 6);

    assert_eq!(est.update_bandwidth_estimation(-50.0, 2.0), -50.0);
    assert_eq!(est.sudden_change_state, 6);
}

#[test]
fn test_update_bwe_initial_cap_0x2063ba0c8() {
    let mut est = VCRateControlBandwidthEstimator::new();
    est.set_radio_access_technology(3);
    est.set_mode(1);

    // 0x2063ba0d0..0x2063ba0e4: capped at 100_000.0 when est == 0.0
    let ret = est.update_bandwidth_estimation(500_000.0, 1.0);
    assert_eq!(ret, 500_000.0);
    assert_eq!(est.estimated_bandwidth, 100_000.0);
    assert_eq!(est.sudden_change_state, 0);

    let mut est2 = VCRateControlBandwidthEstimator::new();
    est2.set_radio_access_technology(3);
    est2.set_mode(1);
    est2.update_bandwidth_estimation(60_000.0, 1.0);
    assert_eq!(est2.estimated_bandwidth, 60_000.0);
    assert_eq!(est2.sudden_change_state, 0);
}

#[test]
fn test_update_bwe_normal_ewma_0x2063ba278() {
    let mut est = VCRateControlBandwidthEstimator::new();
    est.set_radio_access_technology(3);
    est.set_mode(1);
    est.set_estimated_bandwidth(1_000_000.0);
    est.set_server_bag_params(&ServerBagBwEstimationParams {
        agg_mode_enabled: false,
        min_probing_sequence_size: 250,
        sudden_bwd_percentage: 0.5,
        min_fast_bwd_sample_count: 2,
    });

    // 1_100_000 is within +50% and diff 100_000 <= margin 200_000 -> EWMA 0.9*1e6 + 0.1*1.1e6 = 1_010_000
    est.update_bandwidth_estimation(1_100_000.0, 2.0);
    assert!((est.estimated_bandwidth - 1_010_000.0).abs() < 1e-6);
    assert_eq!(est.sudden_change_state, 1);
    assert_eq!(est.bandwidth_estimation_state, 0);
}

#[test]
fn test_update_bwe_sudden_up_0x2063ba314() {
    let mut est = VCRateControlBandwidthEstimator::new();
    est.set_radio_access_technology(3);
    est.set_mode(1);
    est.set_estimated_bandwidth(1_000_000.0);
    est.set_server_bag_params(&ServerBagBwEstimationParams {
        agg_mode_enabled: false,
        min_probing_sequence_size: 250,
        sudden_bwd_percentage: 0.2,
        min_fast_bwd_sample_count: 2,
    });

    // RAT 3 -> min_samples_count = 3, duration threshold not applied
    est.update_bandwidth_estimation(2_000_000.0, 1.0);
    assert_eq!(est.bandwidth_estimation_state, 1);
    assert_eq!(est.sudden_change_state, 2);
    assert_eq!(est.estimated_bandwidth, 1_000_000.0);

    est.update_bandwidth_estimation(2_000_000.0, 1.1);
    assert_eq!(est.bandwidth_estimation_state, 2);
    assert_eq!(est.sudden_change_state, 2);
    assert_eq!(est.estimated_bandwidth, 1_000_000.0);

    est.update_bandwidth_estimation(2_000_000.0, 1.2);
    assert_eq!(est.estimated_bandwidth, 2_000_000.0);
    assert_eq!(est.sudden_change_state, 4);
    assert_eq!(est.bandwidth_estimation_state, 0);
}

#[test]
fn test_update_bwe_sudden_down_rat1_duration_0x2063ba1f4() {
    let mut est = VCRateControlBandwidthEstimator::new();
    est.set_radio_access_technology(1);
    est.set_mode(1);
    est.set_estimated_bandwidth(2_000_000.0);
    est.set_server_bag_params(&ServerBagBwEstimationParams {
        agg_mode_enabled: false,
        min_probing_sequence_size: 250,
        sudden_bwd_percentage: 0.2,
        min_fast_bwd_sample_count: 2,
    });

    // RAT 1 -> min_samples_count = 4, duration_threshold_downward = 4.0
    for i in 0..4 {
        est.update_bandwidth_estimation(500_000.0, 10.0 + (i as f64));
    }
    // At t = 13.0, dt = 3.0 < 4.0 -> does not trigger yet
    assert_eq!(est.estimated_bandwidth, 2_000_000.0);
    assert_eq!(est.bandwidth_estimation_state, -4);
    assert_eq!(est.sudden_change_state, 3);

    // At t = 14.0, dt = 4.0 >= 4.0 -> triggers downward sudden change
    est.update_bandwidth_estimation(500_000.0, 14.0);
    assert_eq!(est.estimated_bandwidth, 500_000.0);
    assert_eq!(est.sudden_change_state, 5);
    assert_eq!(est.bandwidth_estimation_state, 0);
}

#[test]
fn test_update_bwe_rat5_initial_avg_0x2063ba210() {
    let mut est = VCRateControlBandwidthEstimator::new();
    est.set_radio_access_technology(5);
    est.set_mode(1);
    est.is_first_packet = false;
    est.first_diverge_time = 1.0;
    est.set_estimated_bandwidth(200_000.0);

    // sample 300_000 capped by min(300_000, 200_000 * 0.5) = 100_000
    est.update_bandwidth_estimation(300_000.0, 2.0);
    assert_eq!(est.bandwidth_estimation_state, 1);
    assert!(!est.is_first_packet);

    // At t = 8.0, 8.0 - 1.0 == 7.0 >= 7.0 -> finishes initial averaging
    est.update_bandwidth_estimation(300_000.0, 8.0);
    assert!(est.is_first_packet);
    assert_eq!(est.estimated_bandwidth, 100_000.0);
    assert_eq!(est.bandwidth_estimation_state, 0);
    assert_eq!(est.sudden_change_state, 0);
}

#[test]
fn test_calculate_bwe_probing_sequence_0x2063ba414() {
    let mut est = VCRateControlBandwidthEstimator::new();
    est.set_radio_access_technology(3);
    est.set_mode(1);

    // Packet 1 starts probing sequence id=1 at t=1.0
    est.calculate_bandwidth_estimation(1000, 500, true, 1, false, 1.0);
    assert_eq!(est.total_bytes_in_probing_sequence, 0);

    // Packet 2 & 3 accumulate 1000 bytes over 1/64 s (0.015625s) -> 8000 / 0.015625 = 512_000.0 bps
    est.calculate_bandwidth_estimation(1000, 500, true, 1, false, 1.0078125);
    est.calculate_bandwidth_estimation(1000, 500, true, 1, true, 1.015625);

    assert_eq!(est.estimated_bandwidth_uncapped, 512_000.0);
    // Initial est was 0.0 -> capped at 100_000.0
    assert_eq!(est.estimated_bandwidth, 100_000.0);
}

#[test]
fn test_calculate_bwe_over_range_guard_0x2063ba754() {
    let mut est = VCRateControlBandwidthEstimator::new();
    est.set_radio_access_technology(3);
    est.set_mode(1);
    est.set_estimated_bandwidth(100_000.0);

    // 3 short sequences (duration = 1/256 s = 0.00390625 < 0.008, bytes = 1000 -> 2_048_000.0 bps).
    // Close each sequence by advancing packet_id (loc_2063ba4e8) so [x20, #0x8] is evaluated once per sequence.
    est.calculate_bandwidth_estimation(1001, 500, true, 1, false, 1.0);
    est.calculate_bandwidth_estimation(1001, 500, true, 1, false, 1.001953125);
    est.calculate_bandwidth_estimation(1001, 500, true, 1, false, 1.00390625);

    // Closing seq 1 -> over_range_count = 1 (< max 3), passes 0.0 -> sudden_change_state = 6
    est.calculate_bandwidth_estimation(1002, 500, true, 2, false, 2.0);
    assert_eq!(est.over_range_count, 1);
    assert_eq!(est.sudden_change_state, 6);
    assert_eq!(est.estimated_bandwidth, 100_000.0);

    est.calculate_bandwidth_estimation(1002, 500, true, 2, false, 2.001953125);
    est.calculate_bandwidth_estimation(1002, 500, true, 2, false, 2.00390625);
    // Closing seq 2 -> over_range_count = 2 (< max 3)
    est.calculate_bandwidth_estimation(1003, 500, true, 3, false, 3.0);
    assert_eq!(est.over_range_count, 2);
    assert_eq!(est.sudden_change_state, 6);
    assert_eq!(est.estimated_bandwidth, 100_000.0);

    est.calculate_bandwidth_estimation(1003, 500, true, 3, false, 3.001953125);
    est.calculate_bandwidth_estimation(1003, 500, true, 3, false, 3.00390625);
    // Closing seq 3 with non-probing packet -> over_range_count = 3 (>= max 3), passes 2_048_000.0
    // Since 2_048_000.0 >> 100_000.0, UpdateBandwidthEstimation enters upward ramp (sudden_change_state = 2, state = 1)
    est.calculate_bandwidth_estimation(1004, 500, false, 0, false, 4.0);
    assert_eq!(est.over_range_count, 3);
    assert_eq!(est.sudden_change_state, 2);
    assert_eq!(est.bandwidth_estimation_state, 1);
    assert_eq!(est.estimated_bandwidth, 100_000.0);
}

#[test]
fn test_calculate_bwe_with_send_time_0x2063b9c54() {
    let mut est = VCRateControlBandwidthEstimator::new();
    est.set_radio_access_technology(3);
    est.set_mode(1);

    // First packet initializes and starts sequence 10
    est.calculate_bandwidth_estimation_with_send_time(1.0, 0.0, 500, true, 10, 1);
    // Accumulate 1000 bytes: send_duration = 0.001 (send_bwe = 8_000_000 < 10_000_000), rx_duration = 0.010 (bwe = 800_000)
    est.calculate_bandwidth_estimation_with_send_time(1.005, 0.0005, 500, true, 10, 1);
    est.calculate_bandwidth_estimation_with_send_time(1.010, 0.0010, 500, true, 10, 1);
    // Non-probing packet closes sequence 10
    est.calculate_bandwidth_estimation_with_send_time(1.020, 0.0020, 500, false, 11, 0);

    assert!((est.estimated_bandwidth_uncapped - 800_000.0).abs() < 1e-6);
    // send_bwe < 10_000_000.0 forces final_bwe = 0.0 -> state 6, est remains 0.0
    assert_eq!(est.sudden_change_state, 6);
    assert_eq!(est.estimated_bandwidth, 0.0);
}

#[test]
fn test_map_mode1_default_estimator_0x2063bab60() {
    let mut map = VCRateControlBandwidthEstimatorMap::new();
    map.set_mode(1);
    map.set_radio_access_technology(3);

    {
        let e1 = map.bandwidth_estimator_mut(42, true, false).unwrap();
        e1.set_estimated_bandwidth(1_234_000.0);
    }
    // Mode 1 returns the default estimator regardless of estimator_id
    let e2 = map.bandwidth_estimator_mut(99, true, false).unwrap();
    assert_eq!(e2.estimated_bandwidth, 1_234_000.0);
    assert_eq!(map.estimated_bandwidth_with_arrival_time(10.0), 1_234_000.0);
}

#[test]
fn test_collector_quantization_and_staleness_0x205ed4870_0x205ed38fc() {
    let mut map = VCRateControlBandwidthEstimatorMap::new();
    map.set_mode(1);
    map.set_radio_access_technology(3);
    {
        let est = map.bandwidth_estimator_mut(1, true, false).unwrap();
        est.estimated_bandwidth = 1_534_999.0;
        est.estimated_bandwidth_uncapped = 2_000_000.0;
        est.bandwidth_estimation_state = 1;
    }

    let out = compute_bw_estimation_from_map(&mut map, 1, 0, 10.0).unwrap();
    assert_eq!(
        out,
        CollectorBandwidthOutput {
            // 1_534_999 -> 1534 kbps -> & 0x7ffff8 = 1528 -> +1 (state==1) = 1529 -> *1000 = 1_529_000
            shared_estimated_bandwidth: 1_529_000,
            shared_estimated_bandwidth_uncapped: 2_000_000,
        }
    );

    assert_eq!(
        bandwidth_estimation_kbps_for_stats(out.shared_estimated_bandwidth, 1, 40.0, 20.0),
        1529
    );
    // Mode 1 with dt >= 30.0 zeroes output
    assert_eq!(
        bandwidth_estimation_kbps_for_stats(out.shared_estimated_bandwidth, 1, 50.0, 20.0),
        0
    );
    // Collector mode 5 skips ComputeBWEstimation
    assert_eq!(compute_bw_estimation_from_map(&mut map, 5, 0, 10.0), None);
}
