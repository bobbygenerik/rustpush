//! Chunk 2: StabilizedNOWRD UpdateInternalStatistics 0x2061fd3b4 and the Base helpers it calls.
use rustpush::vcrc::{MediaController, PacketLossInput, PacketLossOutput, VcrcState, VcrcStatistics};
use rustpush::vcrc_config::mode1_camera_defaults;

fn st() -> VcrcState { VcrcState::new(mode1_camera_defaults()) }

fn report(ts: u16, time: f64) -> VcrcStatistics {
    VcrcStatistics { time, timestamp: ts, ..Default::default() }
}

struct Mc { settings: (f64, f64, f64, f64), audio_only: bool, rcc: (u32, f64), plr: f64 }
impl MediaController for Mc {
    fn is_audio_only(&self) -> bool { self.audio_only }
    fn last_video_refresh_frame_time(&self) -> f64 { 0.0 }
    fn compute_packet_loss(&mut self, _i: &PacketLossInput) -> PacketLossOutput {
        PacketLossOutput { packet_loss_rate: Some(self.plr), ..Default::default() }
    }
    fn media_queue_settings(&self) -> (f64, f64, f64, f64) { self.settings }
    fn media_queue_rate_change_counter(&self) -> (u32, f64) { self.rcc }
    fn traffic_type(&self) -> u8 { 0 }
    fn is_video_stopped(&self) -> bool { false }
    fn set_min_target_bitrate(&mut self, _bps: u32) {}
    fn set_should_disable_large_frame_requests_when_initial_ramp_up(&mut self, _v: bool) {}
    fn set_rate_limited_max_time_exceeded(&mut self, _v: bool) {}
}

#[test]
fn first_report_only_primes_and_returns_false() {
    let mut s = st();
    let mut r = report(1000, 1.0);
    r.bandwidth_estimation = 3000;
    r.burst_loss = 4;
    assert!(!s.update_internal_statistics(&r, None, 0.0));
    assert!(s.is_first_timestamp_arrived);
    assert_eq!(s.previous_timestamp, 1000);
    assert_eq!(s.first_feedback_arrival_time, 1.0);
    // 0x2061fd400..550 copies happen before the first-report branch.
    assert_eq!(s.remote_bandwidth_estimation, 3000);
    assert_eq!(s.worst_recent_burst_loss, 4);
}

#[test]
fn timestamp_rejection_and_wrap() {
    let mut s = st();
    s.update_internal_statistics(&report(0xff00, 1.0), None, 0.0);
    assert!(!s.update_internal_statistics(&report(0xff00, 1.1), None, 0.0)); // delta 0
    assert!(!s.update_internal_statistics(&report(0x7f00, 1.1), None, 0.0)); // delta 0x8000
    assert!(s.update_internal_statistics(&report(0x0100, 1.2), None, 0.0)); // wrapped forward
    assert_eq!(s.timestamp_wrap_around_counter, 1);
    assert_eq!(s.rate_control_time, 0x100 as f64 / 1024.0 + 64.0);
}

#[test]
fn rtt_ewma_and_min_envelope() {
    let mut s = st();
    s.update_internal_statistics(&report(1, 1.0), None, 0.0);
    let mut r = report(1025, 2.0);
    r.round_trip_time = 0.1;
    s.update_internal_statistics(&r, None, 0.0);
    assert_eq!(s.round_trip_time, 0.1);
    assert_eq!(s.round_trip_time_min_envelope, 0.1);
    let mut r = report(2049, 3.0);
    r.round_trip_time = 0.2;
    s.update_internal_statistics(&r, None, 0.0);
    let rtt = 0.1 * 0.9 + 0.2 * 0.1;
    assert!((s.round_trip_time - rtt).abs() < 1e-12);
    // rtt rose and previousRTT (0) − rtt is not above o_1e0: envelope unchanged, previous set.
    assert_eq!(s.round_trip_time_min_envelope, 0.1);
    assert_eq!(s.previous_round_trip_time, rtt);
    assert_eq!(s.worst_recent_round_trip_time, rtt);
}

#[test]
fn rtt_from_timestamps_and_missing_queuing_delay() {
    let mut s = st();
    s.update_internal_statistics(&report(1, 1.0), None, 0.0);
    // No RTT, no queuing delay: echoed set to 0xffff, RTT untouched.
    let mut r = report(1025, 2.0);
    r.echoed_timestamp = 5;
    s.update_internal_statistics(&r, None, 0.0);
    assert_eq!(s.echoed_timestamp, 0xffff);
    assert_eq!(s.round_trip_time, 0.0);
    // receiveTs − echoed − qd, in 1/1024 s.
    let mut r = report(2049, 3.0);
    r.queuing_delay = 10;
    let rx = rustpush::vcrc::base::get_timestamp_from_micro_time(3.0) as u32;
    r.echoed_timestamp = (rx.wrapping_sub(110) & 0xffff) as u32;
    s.update_internal_statistics(&r, None, 0.0);
    assert_eq!(s.most_recent_round_trip_time, 100.0 / 1024.0);
}

#[test]
fn loss_event_seeds_and_averages() {
    let mut s = st();
    s.round_trip_time = 0.1;
    s.rate_control_time = 5.0;
    s.packet_loss_rate_avg_long = -1.0;
    s.packet_loss_rate = 0.05;
    s.update_loss_event();
    assert_eq!(s.packet_loss_rate_avg_long, 0.05);
    assert_eq!(s.loss_event_buffer_index, 1);
    assert_eq!(s.loss_event_buffer[0].time, 5.0);
    s.packet_loss_rate = 0.1;
    s.update_loss_event();
    assert!((s.packet_loss_rate_avg_long - (0.1 * 0.03 + 0.97 * 0.05)).abs() < 1e-12);
    assert!(s.loss_event_buffer[1].bad_trend);
}

#[test]
fn ecn_ce_ratio_over_window() {
    let mut s = st();
    s.update_internal_statistics(&report(1, 1.0), None, 0.0);
    let rct = 103.0 / 1024.0;
    // Window = clamp(o_358·RTT = 0.3, 0.1..0.5). Record 0 is inside it; record 63 is outside
    // and becomes the reference: ect (+0x1a) 100→140, ce (+0x1c) 10→20.
    s.loss_event_buffer[0].time = rct - 0.05;
    s.loss_event_buffer[0].ecn_low = 120;
    s.loss_event_buffer[0].ecn_high = 15;
    s.loss_event_buffer[63].time = rct - 1.0;
    s.loss_event_buffer[63].ecn_low = 100;
    s.loss_event_buffer[63].ecn_high = 10;
    s.loss_event_buffer_index = 1;
    let mut r = report(103, 1.1);
    r.round_trip_time = 0.1;
    r.ecn_stats = (20u32 << 16) | 140;
    s.update_internal_statistics(&r, None, 0.0);
    assert_eq!(s.loss_event_buffer[1].ecn_low, 140);
    assert!((s.ecn_ce_ratio - 10.0 / 40.0).abs() < 1e-12);
    // An empty (time 0) record reached before the window ends gives 0.
    s.loss_event_buffer[63].time = 0.0;
    let mut r = report(104, 1.2);
    r.round_trip_time = 0.1;
    r.ecn_stats = (20u32 << 16) | 140;
    s.update_internal_statistics(&r, None, 0.0);
    assert_eq!(s.ecn_ce_ratio, 0.0);
}

#[test]
fn send_limited_long_enough_marks_unstable() {
    let mut s = st();
    s.current_tier_index = 20;
    s.target_bitrate = 2_000_000;
    let mut mc = Mc { settings: (0.0, 1_000_000.0, 0.0, 0.0), audio_only: false, rcc: (0, 0.0), plr: 0.0 };
    s.update_internal_statistics(&report(1, 1.0), Some(&mut mc), 0.0);
    s.update_internal_statistics(&report(1025, 2.0), Some(&mut mc), 0.0);
    assert!(s.is_send_bitrate_limited);
    assert_eq!(s.send_bitrate_limited_start_time, 1025.0 / 1024.0);
    assert_eq!(s.actual_bitrate, 1_000_000);
    // Recovered after more than o_110 (1.0 s): SetBitrateUnstable bumps the counter.
    mc.settings.1 = 1_900_000.0;
    let before = s.rate_change_counter;
    s.update_internal_statistics(&report(1025 + 2048, 4.0), Some(&mut mc), 7.0);
    assert!(!s.is_send_bitrate_limited);
    assert_eq!(s.rate_change_counter, before + 1);
    assert_eq!(s.rate_change_system_time, 7.0); // o_208 == 1
}

#[test]
fn check_paused_transitions() {
    let mut s = st();
    s.state = 3;
    s.paused = true;
    s.rate_control_time = 4.0;
    s.check_paused();
    assert_eq!(s.state, 0);
    assert_eq!(s.pause_start_time, 4.0);
    s.rate_control_time = 4.0 + 10.5; // > o_2a0 (10 s) while still paused
    s.check_paused();
    assert_eq!(s.state, 3);
    assert!(!s.paused);
}
