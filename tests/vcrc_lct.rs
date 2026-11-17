//! Chunk 4: LayeredContinuousTier init/Configure, RampUp/RampDown, the state machine and
//! DoRateControl, plus an end-to-end run on clean feedback.
use rustpush::vcrc::{MediaController, PacketLossInput, PacketLossOutput, VcrcState, VcrcStatistics};
use rustpush::vcrc_config::{mode1_camera_defaults, TIER_BITRATES};

#[derive(Default)]
struct Mc { min_target: u32, out_video: f64, audio_only: bool, rate_limited: Option<bool>, large_frames: Option<bool> }
impl MediaController for Mc {
    fn is_audio_only(&self) -> bool { self.audio_only }
    fn last_video_refresh_frame_time(&self) -> f64 { 0.0 }
    fn compute_packet_loss(&mut self, _i: &PacketLossInput) -> PacketLossOutput {
        PacketLossOutput { packet_loss_rate: Some(0.0), packet_loss_rate_video: Some(0.0),
            packet_loss_rate_audio: Some(0.0), most_burst_loss: Some(0), received_bitrate: None }
    }
    fn media_queue_settings(&self) -> (f64, f64, f64, f64) { (0.0, self.out_video, 0.0, 0.0) }
    fn media_queue_rate_change_counter(&self) -> (u32, f64) { (u32::MAX, 0.0) }
    fn traffic_type(&self) -> u8 { 0 }
    fn is_video_stopped(&self) -> bool { false }
    fn set_min_target_bitrate(&mut self, bps: u32) { self.min_target = bps; }
    fn set_should_disable_large_frame_requests_when_initial_ramp_up(&mut self, v: bool) { self.large_frames = Some(v); }
    fn set_rate_limited_max_time_exceeded(&mut self, v: bool) { self.rate_limited = Some(v); }
}

fn fresh(mc: &mut Mc) -> VcrcState {
    VcrcState::new_layered_continuous_tier(mode1_camera_defaults(), Some(mc), 0.0)
}

#[test]
fn creation_matches_init_chain_and_configure() {
    let mut mc = Mc::default();
    let s = fresh(&mut mc);
    assert_eq!(s.state, 1); // Configure(initial = 1) from 0x205ead9f4
    assert_eq!(s.current_tier_index, 17);
    assert_eq!(s.target_bitrate, 502_000);
    assert_eq!(s.target_bitrate_continuous, 502_000.0);
    assert_eq!(s.target_bitrate_tier, 502_000);
    assert_eq!(s.packet_loss_rate_threshold, 0.15); // config+0x2e8
    assert_eq!(s.packet_loss_rate_avg_long, -1.0);
    assert_eq!(s.congestion_severity, 1.0);
    assert!(s.should_ramp_up_skip_nowrd_stabilization);
    assert!(s.bytes_in_flight_adaptation_enabled);
    assert!(s.owrd_list.span_too_short);
    assert_eq!(s.bytes_in_flight_state, -1);
    assert_eq!(mc.min_target, TIER_BITRATES[0]);
}

#[test]
fn ramp_up_and_down_math() {
    let mut mc = Mc::default();
    let mut s = fresh(&mut mc);
    // State 1: time constant = RTT min envelope; speed clamps to 2.0.
    s.round_trip_time_min_envelope = 0.05;
    s.rate_control_time = 1.0;
    let up = s.lct_ramp_up(0.0);
    assert_eq!(up, (502_000.0 * 1.159f64.powf(1.0 * 2.0)) as u32);
    assert_eq!(s.speed_factor, 2.0);
    // Low range: linear 32000 bps per (s × speed).
    let mut s = fresh(&mut mc);
    s.target_bitrate = 100_000;
    s.target_bitrate_continuous = 100_000.0;
    s.round_trip_time_min_envelope = 1.0;
    s.rate_control_time = 0.5;
    assert_eq!(s.lct_ramp_up(0.0), (100_000.0 + 0.5 * 1.0 * 32000.0) as u32);
    // Ramp-down above 228 kbps: divide by 1.159^3 (no flags → factor 3 × severity 1).
    let mut s = fresh(&mut mc);
    s.target_bitrate = 1_000_000;
    s.target_bitrate_continuous = 1_000_000.0;
    assert_eq!(s.lct_ramp_down(0.0), (1_000_000.0 / 1.159f64.powf(3.0)) as u32);
    // Below 228 kbps: subtract 16000 × factor in the 50k..132k band (factor 1 below tier[o_30]).
    let mut s = fresh(&mut mc);
    s.target_bitrate = 100_000;
    s.target_bitrate_continuous = 100_000.0;
    let f = if 100_000.0 < TIER_BITRATES[3] as f64 { 1.0 } else { 3.0 };
    assert_eq!(s.lct_ramp_down(0.0), (100_000.0 - 16000.0 * f) as u32);
}

#[test]
fn set_target_bitrate_maps_tiers() {
    let mut mc = Mc::default();
    let mut s = fresh(&mut mc);
    s.rate_control_time = 7.0;
    s.set_target_bitrate(TIER_BITRATES[20] + 1);
    assert_eq!(s.current_tier_index, 20);
    assert_eq!(s.target_bitrate_tier, TIER_BITRATES[20]);
    s.set_target_bitrate(TIER_BITRATES[18]);
    assert_eq!(s.current_tier_index, 18);
    assert_eq!(s.last_ramp_down_time_due_to_feedback, 7.0);
    s.set_target_bitrate(u32::MAX);
    assert_eq!(s.current_tier_index, 31); // config+0x14
}

#[test]
fn steady_state_ramp_down_updates_wall_and_goes_to_6() {
    let mut mc = Mc::default();
    let mut s = fresh(&mut mc);
    s.state = 2;
    s.target_bitrate = 2_000_000;
    s.target_bitrate_continuous = 2_000_000.0;
    s.should_ramp_down_flag = true;
    s.update_target_bitrate_vcrc(0.0);
    assert_eq!(s.state, 6);
    assert_eq!(s.bandwidth_wall, 2_000_000.0);
    assert_eq!(s.bandwidth_wall_confidence, 1);
    let expect = (2_000_000.0 / 1.159f64.powf(3.0)) as u32;
    assert_eq!(s.target_bitrate, expect);
    // From 6, a ramp-up returns to 2.
    s.should_ramp_down_flag = false;
    s.should_ramp_up_flag = true;
    s.update_target_bitrate_vcrc(0.0);
    assert_eq!(s.state, 2);
}

fn report(k: u32) -> VcrcStatistics {
    VcrcStatistics { time: 1000.0 + k as f64 * 0.1, timestamp: ((k * 102) & 0xffff) as u16,
        round_trip_time: 0.05, ..Default::default() }
}

#[test]
fn clean_network_ramps_up_through_initial_state() {
    let mut mc = Mc { out_video: 1e9, ..Default::default() };
    let mut s = fresh(&mut mc);
    let mut last = s.target_bitrate;
    let mut reached_2 = None;
    for k in 0..600u32 {
        s.do_rate_control(&report(k), Some(&mut mc), 1000.0 + k as f64 * 0.1);
        assert!(s.target_bitrate >= last, "dropped at report {k}");
        last = s.target_bitrate;
        if s.state == 2 && reached_2.is_none() { reached_2 = Some(k); }
    }
    let k2 = reached_2.expect("never left the initial ramp");
    assert!(s.is_first_initial_ramp_up_done);
    assert!(s.target_bitrate > TIER_BITRATES[26], "target {}", s.target_bitrate);
    assert_eq!(mc.rate_limited, Some(false));
    eprintln!("state 2 at report {k2}; after 60 s: {} bps (tier {})", s.target_bitrate, s.current_tier_index);
}

#[test]
fn audio_only_is_not_handled_here() {
    let mut mc = Mc { audio_only: true, ..Default::default() };
    let mut s = fresh(&mut mc);
    assert!(!s.do_rate_control(&report(1), Some(&mut mc), 0.0));
}

#[test]
fn low_bandwidth_optimization_steps_one_tier_on_overshoot() {
    // config+0x18e = 1: a BWE-overshoot ramp-down (rampDownStatus 0x10) below tier
    // config+0x20 drops exactly one tier (0x205f972b0..2e8) instead of dividing.
    let mut mc = Mc::default();
    let mut s = fresh(&mut mc);
    assert_eq!(s.config.o_18e, 1);
    s.ramp_down_status = 0x10;
    assert_eq!(s.lct_ramp_down(0.0), TIER_BITRATES[16]);
}
