#[path = "../src/audio_receive_diagnostics.rs"]
mod diagnostics;

use diagnostics::{frame_layout, AudioPacketDiagnostics, ClockSnapshot, PacketFormat, PacketSample};
use std::time::Duration;

fn sample(sequence: u16, timestamp: u32, frames: usize) -> PacketSample {
    PacketSample {
        sequence,
        timestamp,
        arrival_sec: timestamp as f64 / 24_000.0,
        marker: false,
        payload_bytes: 81 * frames,
        primary_bytes: Some(81 * frames),
        format: PacketFormat {
            configuration: 1,
            stream_group: 2,
            negotiated_stream_id: None,
            outer_payload: 108,
            primary_payload: Some(108),
            red_blocks: Some(0),
        },
        layout: frame_layout(&vec![vec![80; 81]; frames].concat()),
        clock: ClockSnapshot::default(),
    }
}

#[test]
fn counts_complete_frames_without_reading_or_returning_audio_content() {
    let layout = frame_layout(&[2, 0xaa, 0xbb, 1, 0xcc, 3, 1, 2, 3]).unwrap();
    assert_eq!(layout.frames, 3);
    assert_eq!((layout.min_bytes, layout.max_bytes, layout.empty_frames), (1, 3, 0));
    // Existing framing accepts length zero; report it separately, never call it DTX.
    let zero = frame_layout(&[0]).unwrap();
    assert_eq!((zero.frames, zero.empty_frames), (1, 1));
    assert!(frame_layout(&[]).is_none());
    assert!(frame_layout(&[2, 1]).is_none());
    assert!(frame_layout(&[1, 1, 4, 1]).is_none());
}

#[test]
fn reports_one_three_two_frames_and_a_configuration_change() {
    let mut diagnostics = AudioPacketDiagnostics::default();
    let mut sequence = 0;
    let mut timestamp = 0;
    let mut now_ms = 0;
    for frames in [1, 3, 2] {
        let packet = sample(sequence, timestamp, frames);
        let observation = diagnostics.observe(Duration::from_millis(now_ms), packet);
        assert!(observation.changed);
        assert!(observation.detail);
        let summary = observation.summary;
        if frames == 3 {
            let summary = summary.unwrap();
            assert_eq!(summary.frame_counts[1], 1);
            assert_eq!(summary.frame_counts[3], 1);
        }
        if frames == 2 {
            assert_eq!(summary.unwrap().frame_counts[2], 1);
        }
        sequence += 1;
        timestamp += 480 * frames as u32;
        now_ms += 5_000;
    }
    let mut packet = sample(sequence, timestamp, 2);
    packet.format.configuration = 2;
    assert!(diagnostics.observe(Duration::from_millis(now_ms), packet).changed);
}

#[test]
fn preserves_wrap_reordering_and_duplicate_timing_in_measurements() {
    let mut diagnostics = AudioPacketDiagnostics::default();
    let mut first = sample(u16::MAX, u32::MAX - 239, 1);
    first.arrival_sec = 1.0;
    diagnostics.observe(Duration::ZERO, first);
    let mut next = sample(0, 240, 1);
    next.arrival_sec = 1.06;
    let observation = diagnostics.observe(Duration::from_millis(60), next);
    assert_eq!(observation.delta.sequence_step, Some(1));
    assert_eq!(observation.delta.timestamp_step, Some(480));
    assert!((observation.delta.arrival_step_sec.unwrap() - 0.060).abs() < 1e-12);
    let reordered = diagnostics.observe(Duration::from_millis(61), first);
    assert_eq!(reordered.delta.sequence_step, Some(-1));
    assert_eq!(reordered.delta.timestamp_step, Some(-480));
    assert!(reordered.delta.arrival_step_sec.unwrap() < 0.0);
    let duplicate = diagnostics.observe(Duration::from_millis(62), first);
    assert_eq!(duplicate.delta.sequence_step, Some(0));
    assert_eq!(duplicate.delta.timestamp_step, Some(0));
}

#[test]
fn alternating_formats_cannot_flood_logs_or_lose_summary_counts() {
    let mut diagnostics = AudioPacketDiagnostics::default();
    let mut details = 0;
    let mut summaries = 0;
    let mut accounted = 0;
    let mut unknown = 0;
    let mut other = 0;
    for i in 0..=1000 {
        let mut packet = sample(i as u16, i * 240, [1, 3, 6][(i % 3) as usize]);
        if i % 7 == 0 { packet.layout = None; }
        let observation = diagnostics.observe(Duration::from_millis(i as u64 * 10), packet);
        if i < 1000 && observation.detail { details += 1; }
        if let Some(summary) = observation.summary {
            summaries += 1;
            accounted += summary.packets;
            unknown += summary.unknown_frames;
            other += summary.other_frames;
            assert_eq!(summary.packets,
                summary.frame_counts.iter().sum::<u32>() + summary.other_frames + summary.unknown_frames);
        }
    }
    assert_eq!(details, 16); // Eight at startup, eight after five seconds.
    assert_eq!(summaries, 10);
    assert_eq!(accounted, 1001);
    assert_eq!(unknown, 143); // Multiples of seven in 0..=1000.
    assert_eq!(other, 286); // 333 six-frame packets, minus 47 unknowns.
}
