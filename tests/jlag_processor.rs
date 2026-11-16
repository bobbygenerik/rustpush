//! Unit tests for AVConference JLagProcessor ported from iOS 27.0.1 (24A446, iPhone17,5),
//! transliterated from `FaceTimeJLagProcessorTest.kt` (all 21 test cases).
//!
//! RULE: All expected values derived by hand from disassembly literals before running.

use rustpush::jlag_processor::{JLagConfig, JLagProcessor, LagPacket};
use std::sync::Arc;

#[test]
fn test_1_initialize_and_new_stream_constants_0x2064a5d30_0x2064a5dc0() {
    let mut proc = JLagProcessor::new();
    let ctx: Arc<dyn std::any::Any + Send + Sync> = Arc::new("testCtx");
    let config = JLagConfig {
        config_flag: 1,
        context: Some(ctx),
    };
    assert!(proc.initialize(Some(&config)));
    assert_eq!(proc.config_flag, 1);
    assert_eq!(
        proc.config_context
            .as_ref()
            .and_then(|c| c.downcast_ref::<&str>())
            .copied(),
        Some("testCtx")
    );

    let p0 = LagPacket {
        sequence_number: 100,
        timestamp: 24000,
        arrival_sec: 1.0,
        field18: 480,
        field20: 480,
        flag24: 0,
        ..Default::default()
    };
    assert!(proc.new_stream(Some(&p0), 24000, 480));
    assert_eq!(proc.ref_seq, 100u16);
    assert_eq!(proc.secondary_ref_seq, 100u16);
    assert_eq!(proc.ref_timestamp, 24000u32);
    assert_eq!(proc.secondary_ref_timestamp, 24000u32);
    assert_eq!(proc.ref_arrival_sec, 1.0);
    assert_eq!(proc.secondary_ref_arrival_sec, 1.0);

    // 0x2066af5c0 = 0.0, 0x2066af5c8 = 0.01
    assert_eq!(proc.avg_lag, 0.0);
    assert_eq!(proc.avg_dev, 0.01);

    // 0x2066af5d0 = 0.0, 0x2066af5d8 = 32767.0
    assert_eq!(proc.stats_lag, 0.0);
    assert_eq!(proc.stats_last_lag, 32767.0);
    assert_eq!(proc.last_lag, 32767.0);
    assert_eq!(proc.last_secondary_lag, 32767.0);

    let stats = proc.get_statistics();
    assert_eq!(stats.lag, 0.0);
    assert_eq!(stats.last_lag, 32767.0);
    assert_eq!(stats.ref_timestamp, 24000u32);
    assert_eq!(stats.wrap_count, 0u32);
}

#[test]
fn test_2_calculate_packet_lag_math_and_wrap_count_0x2064a6fa0() {
    let mut proc = JLagProcessor::new();
    let p0 = LagPacket {
        sequence_number: 10,
        timestamp: 1000,
        arrival_sec: 0.5,
        ..Default::default()
    };
    proc.new_stream(Some(&p0), 1000, 20);

    // Packet 1: 1 second later (timestamp = 2000u, arrival = 1.5s) -> transit = 1.0s, elapsed = 1.0s, lag = 0.0s
    let mut p1 = LagPacket {
        sequence_number: 11,
        timestamp: 2000,
        arrival_sec: 1.5,
        ..Default::default()
    };
    let (pri1, sec1) = proc.calculate_packet_lag(&mut p1);
    assert!((pri1 - 0.0).abs() < 1e-9);
    assert!((sec1 - 0.0).abs() < 1e-9);
    assert!((p1.remote_time - 1.0).abs() < 1e-9);

    // Packet 2: delayed by 50 ms (timestamp = 3000u, arrival = 2.55s) -> elapsed = 2.05s, transit = 2.0s, lag = 0.05s
    let mut p2 = LagPacket {
        sequence_number: 12,
        timestamp: 3000,
        arrival_sec: 2.55,
        ..Default::default()
    };
    let (pri2, _) = proc.calculate_packet_lag(&mut p2);
    assert!((pri2 - 0.05).abs() < 1e-9);

    // Wrap around test: tsDelta drops below lastTimestampDelta (0x2064a6ff4)
    proc.last_timestamp_delta = 4000000000;
    proc.ref_timestamp = 0;
    let mut p_wrap = LagPacket {
        sequence_number: 13,
        timestamp: 100,
        arrival_sec: 1000.0,
        ..Default::default()
    };
    proc.calculate_packet_lag(&mut p_wrap);
    assert_eq!(proc.wrap_count, 1);
    assert_eq!(proc.last_timestamp_delta, 100);
}

#[test]
fn test_3_reset_lag_reference_sets_reset_pending_0x2064a5f48() {
    let mut proc = JLagProcessor::new();
    assert!(!proc.reset_pending);
    proc.reset_lag_reference();
    assert!(proc.reset_pending);
    // Idempotent:
    proc.reset_lag_reference();
    assert!(proc.reset_pending);
}

#[test]
fn test_4_state1_settle_lag_gating_0x2064a655c_0x2066af5a8() {
    let mut proc = JLagProcessor::new();
    let p0 = LagPacket {
        sequence_number: 1,
        timestamp: 1000,
        arrival_sec: 1.0,
        ..Default::default()
    };
    proc.new_stream(Some(&p0), 1000, 20);

    // Anchor packet: initializedSecondary = true, re-zeros lag to 0.0 (0x2064a61f0..0x2064a6218)
    let mut p_anchor = LagPacket {
        sequence_number: 2,
        timestamp: 2000,
        arrival_sec: 2.0,
        ..Default::default()
    };
    let lag_anchor = proc.process(Some(&mut p_anchor), 20, 0, 1);
    assert!((lag_anchor - 0.0).abs() < 1e-9);
    assert!(proc.initialized_secondary);

    // Packet with lag = 0.030 s (> 0.02500000037252903): should NOT update avgLag in State 1 (0x2064a655c)
    let mut p_over = LagPacket {
        sequence_number: 3,
        timestamp: 3000,
        arrival_sec: 3.030,
        ..Default::default()
    };
    let lag_over = proc.process(Some(&mut p_over), 20, 0, 1);
    assert!((lag_over - 0.030).abs() < 1e-9);
    assert_eq!(proc.avg_lag, 0.0);

    // Packet with lag = 0.020 s (<= 0.02500000037252903): updates avgLag and avgDev (0x2064a6578)
    let mut p_under = LagPacket {
        sequence_number: 4,
        timestamp: 4000,
        arrival_sec: 4.050,
        ..Default::default()
    };
    let lag_under = proc.process(Some(&mut p_under), 20, 0, 1);
    assert!((lag_under - 0.020).abs() < 1e-9);
    assert!((proc.avg_lag - 0.020).abs() < 1e-9);
    // avgDev = Math.abs(0.020) + 0.005 = 0.025 (constant 0x2066a3d40)
    assert!((proc.avg_dev - 0.025).abs() < 1e-9);
    assert!((proc.owrd_short - 0.020).abs() < 1e-9);
    assert!((proc.owrd_long - 0.020).abs() < 1e-9);
}

#[test]
fn test_5_state2_fill_uses_fixed_alpha_0_5_0x2064a6584_0x2064a6c24() {
    let mut proc = JLagProcessor::new();
    let p0 = LagPacket {
        sequence_number: 1,
        timestamp: 1000,
        arrival_sec: 1.0,
        ..Default::default()
    };
    proc.new_stream(Some(&p0), 1000, 20);

    let mut p_anchor = LagPacket {
        sequence_number: 2,
        timestamp: 2000,
        arrival_sec: 2.0,
        ..Default::default()
    };
    proc.process(Some(&mut p_anchor), 20, 0, 2);

    proc.avg_lag = 0.010;
    proc.avg_dev = 0.005;

    // alpha = 0.5, lag = 0.020
    // newAvgLag = 0.020 * 0.5 + 0.010 * 0.5 = 0.015
    let mut p1 = LagPacket {
        sequence_number: 3,
        timestamp: 3000,
        arrival_sec: 3.020,
        ..Default::default()
    };
    let lag1 = proc.process(Some(&mut p1), 20, 0, 2);
    assert!((lag1 - 0.020).abs() < 1e-9);
    // newAvgLag = 0.015 > 0.003: 0.003 bias correction at 0x2064a6c88 -> avgLag = 0.012
    assert!((proc.avg_lag - 0.012).abs() < 1e-9);
}

#[test]
fn test_6_record_owrd_short_and_long_filters_0x2064a6cc4() {
    let mut proc = JLagProcessor::new();
    let p0 = LagPacket {
        sequence_number: 1,
        timestamp: 1000,
        arrival_sec: 1.0,
        ..Default::default()
    };
    proc.new_stream(Some(&p0), 1000, 20);
    proc.owrd_short = 0.010;
    proc.owrd_long = 0.020;
    proc.packet_counter = 10;

    proc.record_owrd(0.030);
    // short: 0.010 * 0.9999 + 0.030 * 0.0001 = 0.010002
    assert!((proc.owrd_short - (0.010 * 0.9999 + 0.030 * 0.0001)).abs() < 1e-9);
    // long: 0.020 * 0.9 + 0.030 * 0.1 = 0.021
    assert!((proc.owrd_long - 0.021).abs() < 1e-9);
    let expected_owrd = (0.021 - (0.010 * 0.9999 + 0.030 * 0.0001)) as f32;
    assert!((proc.get_owrd() - expected_owrd).abs() < 1e-5);

    // Spurious lag (> 5.0 s delta at 0x2064a6d94): ignored
    let pre_short = proc.owrd_short;
    let pre_long = proc.owrd_long;
    proc.record_owrd(10.0);
    assert_eq!(proc.owrd_short, pre_short);
    assert_eq!(proc.owrd_long, pre_long);
}

#[test]
fn test_7a_spike_entry_and_reset_of_ring_index_spike_count_0x2064a671c_0x2064a6864() {
    let mut proc = JLagProcessor::new();
    let p0 = LagPacket {
        sequence_number: 100,
        timestamp: 1000,
        arrival_sec: 1.0,
        ..Default::default()
    };
    proc.new_stream(Some(&p0), 1000, 20);
    proc.last_lag = 0.010;
    proc.avg_lag = 0.010;
    proc.avg_dev = 0.005;

    // Seed dirty state to verify 0x2064a6820 str xzr, [x8, #0xf0] zeroes ringIndex and spikeCount
    proc.ring_index = 17;
    proc.spike_count = 22;
    proc.ring_buffer[5] = 99.0;

    // threshold = 4.0 * 0.005 + 0.019999999552965164 + 0.0 = 0.039999999552965164 (0x2064a671c)
    // lag = 0.100, jump = 0.100 - 0.010 = 0.090 > 0.039999999552965164 -> enters spike
    let mut p_spike = LagPacket {
        sequence_number: 101,
        timestamp: 2000,
        arrival_sec: 2.100,
        ..Default::default()
    };
    let lag = proc.process(Some(&mut p_spike), 20, 0, 5);

    assert!((lag - 0.100).abs() < 1e-9);
    assert_eq!(proc.in_spike, 1); // 0x2064a682c stur w9, [x8, #-0x24]
    assert_eq!(proc.ring_index, 0); // 0x2064a6820 str xzr, [x8, #0xf0] (low 32 bits)
    assert_eq!(proc.spike_count, 0); // 0x2064a6820 str xzr, [x8, #0xf0] (high 32 bits)
    assert!((proc.ring_buffer[0] - 0.100).abs() < 1e-9); // 0x2064a6824 str d8, [x8]
    assert_eq!(proc.ring_buffer[5], 0.0); // 0x2064a6808 stp q0, q0, [x8, #0x30]
    assert!((proc.peak_spike_lag - 0.100).abs() < 1e-9); // 0x2064a6800 stp d9, d8
    assert!((proc.spike_threshold - 0.039999999552965164).abs() < 1e-9); // 0x2064a6800 stp d9, d8
    assert!((proc.spike_start_arrival_sec - 2.100).abs() < 1e-9); // 0x2064a6834 stur d0, [x8, #-0x20]
    assert!((proc.spike_start_timestamp_time_sec - 2.0).abs() < 1e-9); // 0x2064a6860 stur d0, [x8, #-0x18]
}

#[test]
fn test_7b_state5_4a_leave_spike_0x2064a664c() {
    let mut proc = JLagProcessor::new();
    let p0 = LagPacket {
        sequence_number: 100,
        timestamp: 1000,
        arrival_sec: 1.0,
        ..Default::default()
    };
    proc.new_stream(Some(&p0), 1000, 20);
    proc.in_spike = 1;
    proc.avg_lag = 0.010;
    proc.spike_threshold = 0.039999999552965164;
    proc.peak_spike_lag = 0.100;
    proc.last_lag = 0.100;

    // lag = 0.040: (0.040 - 0.010) = 0.030 <= 0.039999999552965164 -> LEAVE at 0x2064a664c
    let mut p_leave = LagPacket {
        sequence_number: 101,
        timestamp: 2000,
        arrival_sec: 2.040,
        ..Default::default()
    };
    let lag = proc.process(Some(&mut p_leave), 20, 0, 5);
    assert!((lag - 0.040).abs() < 1e-9);
    assert_eq!(proc.in_spike, 0); // 0x2064a69e4 str wzr, [x19, #0x34]
}

#[test]
fn test_7c_state5_4c_n_lt_5_stores_ring_n_and_runs_timeout_peak_0x2064a6660() {
    let mut proc = JLagProcessor::new();
    let p0 = LagPacket {
        sequence_number: 100,
        timestamp: 1000,
        arrival_sec: 1.0,
        ..Default::default()
    };
    proc.new_stream(Some(&p0), 1000, 20);
    proc.in_spike = 1;
    proc.spike_count = 0;
    proc.avg_lag = 0.010;
    proc.spike_threshold = 0.039999999552965164;
    proc.peak_spike_lag = 0.100;
    proc.last_lag = 0.100;

    // lag = 0.080: lag - avgLag = 0.070 > 0.039999999552965164 (does not leave on 4a)
    // n = ++spikeCount = 1 < 5 -> stores to ringBuffer[1] and runs timeout/peak
    let mut p_store = LagPacket {
        sequence_number: 101,
        timestamp: 2000,
        arrival_sec: 2.080,
        field20: 20,
        ..Default::default()
    };
    proc.process(Some(&mut p_store), 20, 0, 5);
    assert_eq!(proc.in_spike, 1);
    assert_eq!(proc.spike_count, 1);
    assert!((proc.ring_buffer[1] - 0.080).abs() < 1e-9); // 0x2064a6874 str d8, [x9, w8, uxtw #0x3]
    // PEAK decay: 0.080 * 0.05 + 0.100 * 0.95 = 0.099 (0x2064a6a10)
    assert!((proc.peak_spike_lag - 0.099).abs() < 1e-9);
}

#[test]
fn test_7d_state5_4d_lag_gt_peak_plus_0_01_leaves_spike_0x2064a6664_0x2064a6678() {
    let mut proc = JLagProcessor::new();
    let p0 = LagPacket {
        sequence_number: 100,
        timestamp: 1000,
        arrival_sec: 1.0,
        ..Default::default()
    };
    proc.new_stream(Some(&p0), 1000, 20);
    proc.in_spike = 1;
    proc.spike_count = 4;
    proc.avg_lag = 0.010;
    proc.spike_threshold = 0.039999999552965164;
    proc.peak_spike_lag = 0.100;
    proc.last_lag = 0.100;

    // n = ++spikeCount = 5 (>= 5). peak + 0.01 = 0.110 (0x2064a6670)
    // lag = 0.115 > 0.110 -> 0x2064a6678 falls to 0x2064a667c -> LEAVE at 0x2064a69e4
    let mut p_over = LagPacket {
        sequence_number: 101,
        timestamp: 2000,
        arrival_sec: 2.115,
        ..Default::default()
    };
    proc.process(Some(&mut p_over), 20, 0, 5);
    assert_eq!(proc.in_spike, 0); // left spike
}

#[test]
fn test_7e_state5_4e_5_to_29_stores_ring_n_and_runs_timeout_peak_0x2064a6868() {
    let mut proc = JLagProcessor::new();
    let p0 = LagPacket {
        sequence_number: 100,
        timestamp: 1000,
        arrival_sec: 1.0,
        ..Default::default()
    };
    proc.new_stream(Some(&p0), 1000, 20);
    proc.in_spike = 1;
    proc.spike_count = 4;
    proc.avg_lag = 0.010;
    proc.spike_threshold = 0.039999999552965164;
    proc.peak_spike_lag = 0.100;
    proc.last_lag = 0.100;

    // n = 5 <= 29. lag = 0.105 <= peak + 0.01 (0.110).
    // STORE: ringBuffer[5] = 0.105. PEAK: lag > peak -> peak = 0.105 (0x2064a68c4)
    let mut p_mid = LagPacket {
        sequence_number: 101,
        timestamp: 2000,
        arrival_sec: 2.105,
        field20: 20,
        ..Default::default()
    };
    proc.process(Some(&mut p_mid), 20, 0, 5);
    assert_eq!(proc.in_spike, 1);
    assert_eq!(proc.spike_count, 5);
    assert!((proc.ring_buffer[5] - 0.105).abs() < 1e-9);
    assert!((proc.peak_spike_lag - 0.105).abs() < 1e-9);
}

#[test]
fn test_7f_state5_4f_flat_leaves_spike_0x2064a6a84_0x2064a6b60() {
    let mut proc = JLagProcessor::new();
    let p0 = LagPacket {
        sequence_number: 100,
        timestamp: 1000,
        arrival_sec: 1.0,
        ..Default::default()
    };
    proc.new_stream(Some(&p0), 1000, 20);
    proc.in_spike = 1;
    proc.spike_count = 29;
    proc.avg_lag = 0.010;
    proc.avg_dev = 0.005;
    proc.spike_threshold = 0.039999999552965164;
    proc.peak_spike_lag = 0.100;
    proc.last_lag = 0.080;
    proc.ring_index = 0;
    proc.ring_buffer.fill(0.080);

    // n = ++spikeCount = 30 > 29 -> enters flatness check (0x2064a6a84)
    // ringBuffer all 0.080, lag = 0.080: |sum1 - sum2| = 0.0
    // flatnessThreshold = (1.25 * 0.005 + 0.019999999552965164 + 0.0) * 15.0 = 0.39374999329447746
    // |diff| = 0.0 < 0.3937... -> flat -> LEAVE at 0x2064a69e4
    let mut p_flat = LagPacket {
        sequence_number: 101,
        timestamp: 2000,
        arrival_sec: 2.080,
        ..Default::default()
    };
    proc.process(Some(&mut p_flat), 20, 0, 5);
    assert_eq!(proc.in_spike, 0); // left spike
    assert_eq!(proc.ring_index, 1); // ringIndex advanced (0x2064a6ac4)
}

#[test]
fn test_7g_state5_4f_not_flat_stays_in_spike_0x2064a6b60() {
    let mut proc = JLagProcessor::new();
    let p0 = LagPacket {
        sequence_number: 100,
        timestamp: 1000,
        arrival_sec: 1.0,
        ..Default::default()
    };
    proc.new_stream(Some(&p0), 1000, 20);
    proc.in_spike = 1;
    proc.spike_count = 29;
    proc.avg_lag = 0.010;
    proc.avg_dev = 0.005;
    proc.spike_threshold = 0.039999999552965164;
    proc.peak_spike_lag = 0.150;
    proc.last_lag = 0.100;
    proc.ring_index = 0;
    // sum1 = 15 * 0.050 = 0.75, sum2 = 15 * 0.150 = 2.25 -> |diff| = 1.50 >= 0.3937...
    for i in 0..15 {
        proc.ring_buffer[i] = 0.050;
    }
    for i in 15..30 {
        proc.ring_buffer[i] = 0.150;
    }

    // n = 30 > 29. |diff| = 1.35 >= threshold -> jumps to TIMEOUT_CHECK at 0x2064a6878 without ring[n] store
    // Use field20 = 1 so limit = (1000 * 8.0) / (1 * 20) = 400u, so spikeCount (30) does not time out
    let mut p_not_flat = LagPacket {
        sequence_number: 101,
        timestamp: 2000,
        arrival_sec: 2.100,
        field20: 1,
        ..Default::default()
    };
    proc.process(Some(&mut p_not_flat), 20, 0, 5);
    assert_eq!(proc.in_spike, 1); // still in spike
}

#[test]
fn test_7h_state5_timeout_flag_form_config_flag_1_gt_8s_0x2064a68ac() {
    let mut proc = JLagProcessor::new();
    let p0 = LagPacket {
        sequence_number: 100,
        timestamp: 1000,
        arrival_sec: 1.0,
        ..Default::default()
    };
    proc.new_stream(Some(&p0), 1000, 20);
    proc.config_flag = 1;
    proc.in_spike = 1;
    proc.spike_count = 1;
    proc.spike_start_timestamp_time_sec = 2.0; // spike started at 2.0s
    proc.avg_lag = 0.010;
    proc.spike_threshold = 0.039999999552965164;
    proc.peak_spike_lag = 0.100;
    proc.last_lag = 0.100;

    // ts = 10500u -> curTime = 10.5s -> elapsed = 10.5 - 2.0 = 8.5s > 8.0s (0x2064a68ac)
    let mut p_timeout = LagPacket {
        sequence_number: 101,
        timestamp: 10500,
        arrival_sec: 10.580,
        ..Default::default()
    };
    proc.process(Some(&mut p_timeout), 20, 0, 5);
    assert_eq!(proc.in_spike, 0); // timed out and left spike
}

#[test]
fn test_7i_state5_timeout_count_form_config_flag_0_0x2064a693c() {
    let mut proc = JLagProcessor::new();
    let p0 = LagPacket {
        sequence_number: 100,
        timestamp: 1000,
        arrival_sec: 1.0,
        ..Default::default()
    };
    proc.new_stream(Some(&p0), 1000, 20);
    proc.config_flag = 0;
    proc.in_spike = 1;
    proc.spike_count = 20; // at limit: (1000 * 8.0) / (20 * 20) = 20u
    proc.avg_lag = 0.010;
    proc.spike_threshold = 0.039999999552965164;
    proc.peak_spike_lag = 0.100;
    proc.last_lag = 0.100;

    // n = ++spikeCount = 21 > 20 limit -> timed out and left spike (0x2064a693c)
    let mut p_timeout = LagPacket {
        sequence_number: 101,
        timestamp: 2000,
        arrival_sec: 2.080,
        field20: 20,
        ..Default::default()
    };
    proc.process(Some(&mut p_timeout), 20, 0, 5);
    assert_eq!(proc.in_spike, 0); // left spike

    // Field20 clamp test (field20 <= 0 clamped to 1 at 0x2064a6914):
    let mut proc2 = JLagProcessor::new();
    proc2.new_stream(Some(&p0), 1000, 20);
    proc2.in_spike = 1;
    proc2.spike_count = 5;
    proc2.avg_lag = 0.010;
    proc2.spike_threshold = 0.039999999552965164;
    proc2.peak_spike_lag = 0.100;
    proc2.last_lag = 0.100;
    // field20 = 0 -> clamped to 1 -> limit = (1000 * 8.0) / (1 * 20) = 400u
    // spikeCount = 6 <= 400 -> stays in spike
    let mut p_clamped = LagPacket {
        sequence_number: 101,
        timestamp: 2000,
        arrival_sec: 2.080,
        field20: 0,
        ..Default::default()
    };
    proc2.process(Some(&mut p_clamped), 20, 0, 5);
    assert_eq!(proc2.in_spike, 1);
}

#[test]
fn test_7j_state5_peak_both_branches_0x2064a68bc_0x2064a6a14() {
    let mut proc = JLagProcessor::new();
    let p0 = LagPacket {
        sequence_number: 100,
        timestamp: 1000,
        arrival_sec: 1.0,
        ..Default::default()
    };
    proc.new_stream(Some(&p0), 1000, 20);
    proc.in_spike = 1;
    proc.spike_count = 0;
    proc.avg_lag = 0.010;
    proc.spike_threshold = 0.039999999552965164;
    proc.peak_spike_lag = 0.100;
    proc.last_lag = 0.100;

    // Branch 1: lag > peak (0.108 > 0.100) -> peak becomes 0.108 (0x2064a68c4)
    let mut p_higher = LagPacket {
        sequence_number: 101,
        timestamp: 2000,
        arrival_sec: 2.108,
        field20: 20,
        ..Default::default()
    };
    proc.process(Some(&mut p_higher), 20, 0, 5);
    assert!((proc.peak_spike_lag - 0.108).abs() < 1e-9);

    // Branch 2: lag <= peak (0.080 <= 0.108) -> peak decays: 0.080 * 0.05 + 0.108 * 0.95 = 0.1066 (0x2064a69fc..0x2064a6a10)
    let mut p_lower = LagPacket {
        sequence_number: 102,
        timestamp: 3000,
        arrival_sec: 3.080,
        field20: 20,
        ..Default::default()
    };
    proc.process(Some(&mut p_lower), 20, 0, 5);
    let expected_decay = 0.080 * 0.05 + 0.108 * 0.95;
    assert!((proc.peak_spike_lag - expected_decay).abs() < 1e-9);
}

#[test]
fn test_8_state5_spurious_path_bad_secondary_with_reset_pending_0x2064a6318_0x2064a6360() {
    let mut proc = JLagProcessor::new();
    let p0 = LagPacket {
        sequence_number: 1,
        timestamp: 1000,
        arrival_sec: 1.0,
        ..Default::default()
    };
    proc.new_stream(Some(&p0), 1000, 20);
    proc.last_lag = 0.020;

    // Secondary reference configured
    proc.secondary_ref_seq = 10;
    proc.secondary_ref_timestamp = 10000;
    proc.secondary_ref_arrival_sec = 19.000;
    proc.last_secondary_lag = 0.020;

    // Primary lag > 5.0 (spurious). Secondary lag = 3.0 >= 2.5 (bad secondary, 0x2064a6324 b.pl 0x2064a6354).
    // counter198 is zeroed.
    // If resetPending is true, 0x2064a6358 falls through to switch reference to secondary!
    proc.reset_pending = true;
    let mut p_spurious = LagPacket {
        sequence_number: 11,
        timestamp: 11000,
        arrival_sec: 23.000, // secondaryElapsed = 4.0, transit = 1.0 -> secLag = 3.0 (>= 2.5)
        ..Default::default()
    };
    let res = proc.process(Some(&mut p_spurious), 20, 0, 5);

    assert!((res - 3.0).abs() < 1e-9); // switched to secondary lag
    assert_eq!(proc.ref_seq, 10u16); // reference switched!
    assert!(!proc.reset_pending); // resetPending cleared (0x2064a6458)
    assert_eq!(proc.counter198, 0); // counter198 cleared (0x2064a6450)
}

#[test]
fn test_9_out_of_order_check_exact_boundary_diff16_0x7fff_0x2064a6170_0x2064a6178() {
    let mut proc = JLagProcessor::new();
    let p0 = LagPacket {
        sequence_number: 100,
        timestamp: 1000,
        arrival_sec: 1.0,
        ..Default::default()
    };
    proc.new_stream(Some(&p0), 1000, 20);
    proc.last_lag = 0.030;
    proc.packet_counter = 50;

    // Case A: diff16 == 0x7fff.
    // Apple 0x2064a6174: cmp #0x7fff, diff16; b.hi calculate -> 0x7fff > 0x7fff is FALSE!
    // Enters reordered/out-of-order path: oooCounter increments, returns lastLag (0.030)
    let mut p_exact = LagPacket {
        sequence_number: (100 + 0x7fff) as u16,
        timestamp: 2000,
        arrival_sec: 2.050,
        ..Default::default()
    };
    let res_exact = proc.process(Some(&mut p_exact), 20, 0, 1);
    assert_eq!(res_exact, 0.030);
    assert_eq!(proc.ooo_counter, 1);

    // Case B: diff16 == 0x7ffe < 0x7fff.
    // 0x7fff > 0x7ffe is TRUE -> branches to calculate path: oooCounter reset to 0
    let mut p_normal = LagPacket {
        sequence_number: (100 + 0x7ffe) as u16,
        timestamp: 2000,
        arrival_sec: 2.0,
        ..Default::default()
    };
    proc.process(Some(&mut p_normal), 20, 0, 1);
    assert_eq!(proc.ooo_counter, 0); // oooCounter cleared in calculatePacketLag (0x2064a6fd0)
}

#[test]
fn test_10_new_stream_preserves_state_variables_0x2064a5dc0() {
    let mut proc = JLagProcessor::new();
    // Pre-populate fields that Apple does NOT touch in NewStream
    proc.ooo_counter = 3; // +0x8
    proc.packet_counter = 42; // +0xc
    proc.owrd_short = 0.123; // +0x20
    proc.owrd_long = 0.456; // +0x28
    proc.owrd = 0.789; // +0x30
    proc.in_spike = 1; // +0x34
    proc.spike_start_arrival_sec = 9.99; // +0x38
    proc.spike_start_timestamp_time_sec = 8.88; // +0x40
    proc.spike_threshold = 0.055; // +0x48
    proc.peak_spike_lag = 0.125; // +0x50
    proc.ring_buffer[5] = 0.333; // +0x58..+0x147
    proc.ring_index = 7; // +0x148
    proc.spike_count = 14; // +0x14c
    proc.reset_pending = true; // +0x158
    proc.initialized_secondary = true; // +0x159
    proc.counter198 = 9; // +0x198

    let p = LagPacket {
        sequence_number: 200,
        timestamp: 5000,
        arrival_sec: 5.0,
        ..Default::default()
    };
    proc.new_stream(Some(&p), 1000, 20);

    // Assert all preserved
    assert_eq!(proc.ooo_counter, 3);
    assert_eq!(proc.packet_counter, 42);
    assert_eq!(proc.owrd_short, 0.123);
    assert_eq!(proc.owrd_long, 0.456);
    assert_eq!(proc.owrd, 0.789);
    assert_eq!(proc.in_spike, 1);
    assert_eq!(proc.spike_start_arrival_sec, 9.99);
    assert_eq!(proc.spike_start_timestamp_time_sec, 8.88);
    assert_eq!(proc.spike_threshold, 0.055);
    assert_eq!(proc.peak_spike_lag, 0.125);
    assert_eq!(proc.ring_buffer[5], 0.333);
    assert_eq!(proc.ring_index, 7);
    assert_eq!(proc.spike_count, 14);
    assert!(proc.reset_pending);
    assert!(proc.initialized_secondary);
    assert_eq!(proc.counter198, 9);
}

#[test]
fn test_11_update_averages_3ms_bias_correction_0x2064a6c88_0x2064a6cbc() {
    let mut proc = JLagProcessor::new();
    let p0 = LagPacket {
        sequence_number: 1,
        timestamp: 10000,
        arrival_sec: 1.0,
        ..Default::default()
    };
    proc.new_stream(Some(&p0), 1000, 20);
    proc.avg_lag = 0.005;
    proc.avg_dev = 0.001;

    let p = LagPacket {
        sequence_number: 2,
        timestamp: 10020,
        arrival_sec: 1.025,
        field18: 20,
        field20: 20,
        ..Default::default()
    };
    // With lag = 0.005, alpha = 0.5 in state 2:
    // newAvgLag = 0.005 * 0.5 + 0.005 * 0.5 = 0.005 > 0.003 (constant 0x2066af5b0)
    // Deducts 0.003: avgLag becomes 0.002
    // refTimestamp shifts by (1000 * 0.003) = 3: 10000 - 3 = 9997
    proc.update_averages(0.005, &p, 20, true);
    assert!((proc.avg_lag - 0.002).abs() < 1e-9);
    assert_eq!(proc.ref_timestamp, 9997);
}

#[test]
fn test_12_invalid_arguments_and_flag24_eq_1_return_nan_0x2064a5fd0_0x2064a5ff0() {
    let mut proc = JLagProcessor::new();
    let mut p = LagPacket {
        sequence_number: 1,
        timestamp: 1000,
        arrival_sec: 1.0,
        flag24: 0,
        ..Default::default()
    };
    proc.new_stream(Some(&p), 1000, 20);

    // Null packet:
    assert!(proc.process(None, 20, 0, 1).is_nan());

    // State > 5:
    assert!(proc.process(Some(&mut p), 20, 0, 6).is_nan());

    // Negative samplesPerFrame or queueSamples:
    assert!(proc.process(Some(&mut p), -1, 0, 1).is_nan());
    assert!(proc.process(Some(&mut p), 20, -1, 1).is_nan());

    // flag24 == 1:
    let mut p_pt1 = LagPacket {
        sequence_number: 2,
        timestamp: 1020,
        arrival_sec: 1.020,
        flag24: 1,
        ..Default::default()
    };
    assert!(proc.process(Some(&mut p_pt1), 20, 0, 1).is_nan());
}
