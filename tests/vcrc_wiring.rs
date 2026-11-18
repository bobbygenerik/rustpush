//! The receive parser, real sender counters, queue rates and unchanged port together.
use rustpush::avconference::{sender_rate::{SenderMediaController, SenderRateController}, USE_APPLE_VCRC};
use rustpush::vcrc::{MediaController, PacketLossInput};
use rustpush::feedback_delay::ntp_send_timestamp;

fn hex(s: &str) -> Vec<u8> {
    s.as_bytes().chunks_exact(2).map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()).collect()
}

#[test]
fn captured_feedback_uses_wire_units_and_packed_video() {
    // calltest-20261117/logcat.txt, FTFeedbackRx, 12:39:18.537, 19.553,
    // 20.475 and 21.475. Log raw= is limited to 24 bytes, so the two flags=8f
    // samples omit their connection-stats tail and cannot be full parser fixtures.
    let rows = [
        ("8d00000519ee00e000020014b27f0000f000000000000000", 6638, 2, 20, 45695, 0, 0, 15),
        ("8d0000051e00027c00350016b691000b03c0000000000000", 7680, 53, 22, 46737, 11, 0x3c0, 0),
        ("8d00000521c4040200640002ba3f00380380000000000000", 8644, 100, 2, 47679, 56, 0x380, 0),
        ("8d00000525b1058100950019be3f001402c0000000000000", 9649, 149, 25, 48703, 20, 0x2c0, 0),
    ];
    let mut controller = SenderRateController::new(false, 1000.0);
    for (raw, echo, audio, delay, timestamp, q13, video, burst) in rows {
        let bytes = hex(raw);
        let stats = controller.decode_feedback(u16::from_be_bytes([bytes[0], bytes[1]]), &bytes[4..], 1, 1001.5).unwrap();
        assert_eq!(stats.time, 1001.5);
        assert_eq!(stats.echoed_timestamp, echo);
        assert_eq!(stats.audio_received_packets, audio);
        assert_eq!(stats.queuing_delay, delay);
        assert_eq!(stats.timestamp, timestamp);
        assert_eq!(stats.owrd, q13 as f64 / 8192.0);
        assert_eq!(stats.video_received_packets, video);
        assert_eq!(stats.burst_loss, burst);
        assert_eq!(stats.total_received_kbytes, 0); // native construction, see wiring.md
        assert_eq!((stats.bandwidth_estimation, stats.is_bwe_stable), (0, 0));
    }
    // Both genuinely truncated flags=8f raw samples are rejected; do not invent a tail.
    for raw in ["8f000006219a03f600620012ba2b00420440000000000000",
                "8f00000625b1058100950005be2b001402c0000000000000"] {
        let bytes = hex(raw);
        assert!(controller.decode_feedback(0x8f00, &bytes[4..], 1, 1002.0).is_none());
    }
}

fn feedback(audio: u16, now: f64, echo: u16, bwe: u16, q13: u16, video: u16) -> Vec<u8> {
    [echo, 1234, audio & 0xfff, 1, ntp_send_timestamp(now), q13, video, bwe, 0, 0]
        .into_iter().flat_map(u16::to_be_bytes).collect()
}

#[test]
fn native_feedback_defaults_and_counter_wrap() {
    let mut c = SenderRateController::new(false, 1700000000.0);
    for (audio, expected) in [(1500, 1500), (3000, 3000), (4090, 4090), (4095, 4095), (4, 4100), (4090, 4100), (9, 4105)] {
        let raw = feedback(audio, 1700000001.0, 80, 3001, 0xffff, (63 << 6) | 7);
        let s = c.decode_feedback(0x8d03, &raw, 1, 1700000001.0).unwrap();
        assert_eq!(s.receive_jitter_queue_size, 3);
        assert_eq!(s.audio_received_packets, expected);
        assert_eq!(s.video_received_packets, (63 << 6) | 7);
        assert_eq!((s.bandwidth_estimation, s.is_bwe_stable), (3001, 1));
        assert_eq!(s.owrd, 0.0);
        assert_eq!(s.round_trip_time, 0.0); // native port derives RTT
    }
}

#[test]
fn sender_measures_queue_input_and_output_separately() {
    let mut m = SenderMediaController::new(false, 1000.0);
    m.enqueue(false, 1000, false, 1000.0);
    m.enqueue(true, 100, true, 1000.0);
    m.sent(true, 100, true, Some(51), 1000.0);
    let (iv, ov, ia, oa) = m.media_queue_settings();
    assert_eq!(iv, 1028.0 * 8.0 / 0.484375);
    assert_eq!(ov, 0.0);
    assert_eq!(ia, 148.0 * 8.0 / 0.484375);
    assert_eq!(oa, ia);
    assert_eq!(m.tx_history.sent_audio_packets, 1);
    assert_eq!(m.tx_history.records[0].tx_timestamp, 51);
    m.sent(false, 1000, false, None, 1000.25);
    assert!(m.media_queue_settings().1 > 0.0);
    assert_eq!(m.tx_history.sent_video_packets, 1);
    m.set_time(1001.0);
    assert_eq!(m.media_queue_settings(), (0.0, 0.0, 0.0, 0.0));
}

#[test]
fn default_on_starts_at_502k_and_clean_sent_feedback_ramps_to_native_cap() {
    assert!(USE_APPLE_VCRC);
    let start = 1700000000.0;
    let mut c = SenderRateController::new(false, start);
    assert_eq!(c.state.target_bitrate, 502000); // first SelectVideoBitrate in AVSession::new
    let mut last = c.state.target_bitrate;
    let mut audio = 0u16;
    let mut raised = false;
    for k in 1..=3000u32 {
        let sent = start + k as f64 * 0.02;
        audio = audio.wrapping_add(1);
        c.media.enqueue(true, 100, false, sent);
        c.media.sent(true, 100, false, Some(ntp_send_timestamp(sent)), sent);
        // A camera actually delivering the requested network rate (not an arbitrary 1e9 fake).
        let bytes = (last as f64 * 0.02 / 8.0) as u32;
        for len in std::iter::repeat(1000u16).take((bytes / 1000) as usize)
            .chain(std::iter::once((bytes % 1000) as u16)) {
            c.media.enqueue(false, len, false, sent);
            c.media.sent(false, len, false, None, sent);
        }
        if k % 5 != 0 { continue; }
        let now = sent + 0.05;
        let raw = feedback(audio, now, ntp_send_timestamp(sent), 0, 0, 5 << 6);
        let s = c.decode_feedback(0x8d00, &raw, 1, now).unwrap();
        if let Some(target) = c.on_feedback(&s, now) {
            assert!(target >= last, "clean report {k}: {last} -> {target}");
            assert!(target <= 4084000);
            raised |= target > last;
            last = target;
        }
    }
    assert!(raised);
    assert!(last > 1949000, "target {last}");
}

#[test]
fn sender_history_exposes_measured_audio_loss_and_audio_only_stays_legacy() {
    let mut c = SenderRateController::new(true, 1000.0);
    let raw = feedback(1, 1001.0, 100, 0, 0, 0);
    let s = c.decode_feedback(0x8d00, &raw, 1, 1001.0).unwrap();
    assert_eq!(c.on_feedback(&s, 1001.0), None);
    let mut m = SenderMediaController::new(false, 1000.0);
    let mut result = Default::default();
    for k in 1..=500u32 {
        let t = 1000.0 + k as f64 * 0.02;
        let ts = ntp_send_timestamp(t);
        m.sent(true, 100, false, Some(ts), t);
        if k % 5 == 0 {
            result = m.compute_packet_loss(&PacketLossInput {
                echoed_timestamp: ts, audio_received_packets: k * 9 / 10,
                video_received_packets: (5 << 6) | 1, ..Default::default()
            });
        }
    }
    assert!((result.packet_loss_rate_audio.unwrap() - 0.1).abs() < 0.02);
    assert!((result.packet_loss_rate.unwrap() - 0.1).abs() < 0.02);
    assert_eq!(result.packet_loss_rate_video, Some(0.2));
}

#[test]
fn frozen_positive_owrd_keeps_native_downgrade_and_logs_pre_clear_bits() {
    // Native ShouldRampDown 0x2065c1c5c..ccc treats a positive, frozen OWRD as
    // a condition to reduce rate, even with zero loss. This is not a clean ramp.
    let mut c = SenderRateController::new(false, 1700000000.0);
    let mut saw_frozen = false;
    for k in 1..=60u16 {
        let now = 1700000000.0 + k as f64 * 0.1;
        c.media.enqueue(false, 6500, false, now);
        c.media.sent(false, 6500, false, None, now);
        let stamp = ntp_send_timestamp(now - 0.05);
        c.media.sent(true, 100, false, Some(stamp), now - 0.05);
        let raw = feedback(k, now, stamp, 0, 164, 5 << 6);
        let s = c.decode_feedback(0x8d00, &raw, 1, now).unwrap();
        let previous = c.state.target_bitrate;
        c.on_feedback(&s, now);
        if c.media.last_decision_status.1 & 1 != 0 {
            assert!(c.state.target_bitrate < previous);
            saw_frozen = true;
        }
        assert_eq!((c.state.ramp_up_status, c.state.ramp_down_status), (0, 0));
    }
    assert!(saw_frozen);
}

#[test]
fn absent_video_keeps_the_native_cumulative_counter_guard() {
    let mut c = SenderRateController::new(false, 1000.0);
    let raw = feedback(1, 1001.0, 40, 0, 0, 100);
    assert_eq!(c.decode_feedback(0x8d00, &raw, 0, 1001.0).unwrap().video_received_packets, 100);
    // flags=1 carries only the 12-byte audio/timing body. Native supplies zero
    // to the cumulative unwrap guard, which rejects this old half-cycle value.
    assert_eq!(c.decode_feedback(0x8100, &raw[..12], 0, 1001.1).unwrap().video_received_packets, 100);
}

#[test]
fn notify_gate_matches_avcratecontroller() {
    // __AVCRateController_NotifyTargetBitrateChange 0x205eb261c: during a ramp-up inside one
    // tier, the encoder hears about a new target only every 10 rate changes; a tier change,
    // ramping down (state 6) or a zero tier always notifies.
    use rustpush::avconference::sender_rate::NotifyGate;
    let rc = SenderRateController::new(false, 0.0);
    let mut s = rc.state.clone();
    let mut g = rc.notify_gate;
    assert_eq!((g.last_target, g.last_tier), (502_000, 502_000));
    s.state = 2;
    let mut sent = 0;
    for k in 1..=25u32 {
        s.rate_change_counter += 1;
        s.target_bitrate = 502_000 + k * 3000; // same tier bps
        if g.notify(&s).is_some() { sent += 1; }
    }
    assert_eq!(sent, 2); // counters +10 and +20
    s.target_bitrate_tier = 599_000;
    s.rate_change_counter += 1;
    s.target_bitrate += 1000;
    assert!(g.notify(&s).is_some()); // tier changed
    s.state = 6;
    s.rate_change_counter += 1;
    s.target_bitrate -= 50_000;
    assert!(g.notify(&s).is_some()); // ramping down
    assert!(g.notify(&s).is_none()); // same target, same counter
    let _ = NotifyGate::default();
}
