//! Chunk 3: ShouldRampDown 0x2065c1bb4, ShouldRampUp 0x2061ff134, ShouldFastRampUp
//! 0x2065c19c8, IsBitrateOscillating 0x2062009b0 and their helpers.
use rustpush::vcrc::ramp_down::congestion_level_bitrate_range;
use rustpush::vcrc::VcrcState;
use rustpush::vcrc_config::mode1_camera_defaults;

fn st() -> VcrcState {
    let mut s = VcrcState::new(mode1_camera_defaults());
    s.is_owrd_list_ready = true;
    s
}

#[test]
fn config_values_added_in_chunk_3() {
    let c = mode1_camera_defaults();
    // 0x2066a3970.. q-pair stores at 0x205eaa8f0/8f4; minTierGap 3; send-limited duration 1.0.
    assert_eq!((c.o_60, c.o_68, c.o_70, c.o_78), (0.1, 0.15, 0.06, 0.1));
    assert_eq!((c.o_110, c.o_1b8, c.o_1a0), (1.0, 3, 0.0));
    // PopulateCongestionLevelThresholds 0x2060de4d0; entries 5..=9 match o_3a0..o_3c0.
    assert_eq!(c.o_378[0..5], [-1.0, 4.0, 3.5, 3.0, 2.5]);
    assert_eq!(c.o_378[5..10], [c.o_3a0, c.o_3a8, c.o_3b0, c.o_3b8, c.o_3c0]);
    assert_eq!(c.o_378[19].to_bits(), 0x3fdcccccc0000000);
}

#[test]
fn bitrate_range_boundaries() {
    // 0x2060de464: thresholds 100001, 228001, 597001, 1949001.
    let r: Vec<u32> = [100_000, 100_001, 228_000, 228_001, 597_000, 597_001, 1_949_000, 1_949_001]
        .iter().map(|&b| congestion_level_bitrate_range(b)).collect();
    assert_eq!(r, [0, 1, 1, 2, 2, 3, 3, 4]);
}

#[test]
fn ramp_down_needs_ready_owrd_list() {
    let mut s = st();
    s.is_owrd_list_ready = false;
    s.round_trip_time = 5.0;
    s.current_tier_index = 30;
    assert!(!s.should_ramp_down());
    assert_eq!(s.fast_ramp_down_bitrate_range, s.config.o_148);
}

#[test]
fn ramp_down_frozen_owrd_and_rtt() {
    let mut s = st();
    s.rate_control_time = 10.0;
    s.first_owrd_frozen_time = 7.5; // > config+0x80 (2 s)
    s.nowrd_acc = 0.05;
    assert!(s.should_ramp_down());
    assert_eq!(s.ramp_down_status, 0x1);
    assert_eq!(s.first_owrd_frozen_time, 0.0);
    let mut s = st();
    s.round_trip_time = 2.5;
    s.current_tier_index = 27; // > config+0x28 (26)
    assert!(s.should_ramp_down());
    assert_eq!(s.ramp_down_status, 0x400);
}

#[test]
fn ramp_down_nowrd_in_initial_state_uses_agg_threshold() {
    let mut s = st();
    s.state = 1;
    s.nowrd = 0.07; // > aggNOWRD 0.06 (not send-limited)
    s.nowrd_short = 0.01;
    s.previous_nowrd = 0.07;
    assert!(s.should_ramp_down());
    assert_eq!(s.ramp_down_status & 0x4, 0x4);
    assert_eq!(s.previous_nowrd, 0.07);
    // Send-limited: NOWRD threshold 0.1, so 0.07 does not fire.
    let mut s = st();
    s.state = 1;
    s.is_send_bitrate_limited = true;
    s.nowrd = 0.07;
    s.nowrd_short = 0.01;
    s.previous_nowrd = 0.07;
    assert!(!s.should_ramp_down());
}

#[test]
fn ramp_down_nowrd_accumulation() {
    let mut s = st();
    s.state = 3;
    s.current_tier_index = 20;
    s.nowrd_acc = 0.2; // > NOWRDAcc 0.15 outside the initial state
    assert!(s.should_ramp_down());
    assert_eq!(s.ramp_down_status, 0x8);
    assert!((s.nowrd_acc - 0.05).abs() < 1e-12);
}

#[test]
fn ramp_down_ecn() {
    let mut s = st();
    s.ecn_ce_ratio = 0.2; // > config+0x340 (0.1)
    s.target_bitrate = 500_000; // >= config+0x350 (228000)
    assert!(s.should_ramp_down());
    assert_eq!(s.ramp_down_status, 0x800);
    assert_eq!(s.additional_tier_for_ramp_down, 0); // tier[o_14] != 228000
}

#[test]
fn loss_event_by_bad_trend_and_by_lba() {
    let mut s = st();
    s.current_tier_index = 20;
    s.rate_control_time = 10.0;
    for k in 0..s.config.o_dc as usize {
        s.loss_event_buffer[k].bad_trend = true;
        s.loss_event_buffer[k].time = 10.0 - k as f64 * 0.1;
        s.loss_event_buffer[k].packet_loss_rate = 0.05;
    }
    s.loss_event_buffer_index = s.config.o_dc;
    // Only a few records before an empty one: the ratio stays 0, no LBA.
    assert!(s.should_ramp_down_due_to_loss_event());
    assert!(!s.did_lba_ramp_down);

    let mut s = st();
    s.current_tier_index = 20;
    s.previous_tier_index = 10;
    s.rate_control_time = 10.0;
    s.target_bitrate = 1_000_000;
    s.actual_bitrate = u32::MAX;
    s.packet_loss_rate_avg_long = 0.05;
    for k in 0..64 {
        s.loss_event_buffer[k].time = 10.0 - k as f64 * 0.01;
        s.loss_event_buffer[k].packet_loss_rate = 0.05;
    }
    s.loss_event_buffer_index = 0;
    assert!(s.should_ramp_down_due_to_loss_event());
    assert!(s.did_lba_ramp_down);
    assert_eq!(s.first_loss_event_ramp_down_time, 10.0);
    assert_eq!(s.packet_loss_rate_avg_long_at_lba_start, 0.05);
    assert_eq!(s.loss_event_bandwidth_limit, 1_000_000);
    assert_eq!(s.packet_loss_rate_threshold, s.config.o_2f0.max(0.0));
}

#[test]
fn ramp_up_clean_and_blocked() {
    let mut s = st();
    s.is_stable = true;
    s.rate_control_time = 3.0;
    assert!(s.should_ramp_up(None));
    assert_eq!(s.last_allow_ramp_up_time, 3.0);
    assert_eq!(s.ramp_up_status, 0);

    let mut s = st();
    s.is_stable = true;
    s.is_send_bitrate_limited = true;
    s.last_allow_ramp_up_time = 1.0;
    s.rate_control_time = 20.0;
    assert!(!s.should_ramp_up(None));
    assert_eq!(s.ramp_up_status, 0x8);
    // After config+0xc8 (30 s) with send-limited as the only blocker, ramp up anyway.
    s.ramp_up_status = 0;
    s.rate_control_time = 31.0;
    assert!(s.should_ramp_up(None));
    assert_eq!(s.ramp_up_status, 0);
    assert_eq!(s.last_allow_ramp_up_time, 31.0);
}

#[test]
fn ramp_up_blockers_set_bits() {
    let mut s = st();
    s.nowrd = 0.01; // > config+0xb0
    s.current_tier_index = 31; // >= max tier config+0x14
    s.ecn_ce_ratio = 0.01;
    assert!(!s.should_ramp_up(None));
    assert_eq!(s.ramp_up_status, 0x2 | 0x20 | 0x1000 | 0x8000);
}

#[test]
fn fast_ramp_up() {
    let mut s = st();
    s.is_remote_bandwidth_estimation_stable = true;
    s.current_tier_index = 20;
    s.remote_bandwidth_estimation = s.tier(23) / 1000 + 1; // above tier[20 + minTierGap]
    s.rate_control_time = 100.0;
    assert!(s.should_fast_ramp_up());
    s.remote_bandwidth_estimation = s.tier(23) / 1000;
    assert!(!s.should_fast_ramp_up());
}

#[test]
fn history_and_oscillation() {
    let mut s = st();
    s.config.o_186 = 1;
    for t in [20, 22, 20, 22, 20, 22, 20, 22] {
        s.current_tier_index = t;
        s.update_vc_rate_control_history(0);
    }
    assert_eq!(s.current_history_size, 8);
    assert_eq!(s.current_history_index, 7);
    assert_eq!(s.history[0].tier_index, 20);
    let (c1e8, c1ec) = (s.config.o_1e8 as i32, s.config.o_1ec as i32);
    // Alternating between two tiers: changes reach 1 + 7, distinct stays 2.
    let expect = 8 > c1e8 && 2 < c1ec;
    assert_eq!(s.is_bitrate_oscillating(20), expect);
}
