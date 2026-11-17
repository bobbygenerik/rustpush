//! Sender-side packet loss: VCMediaQueue tx history + FindPacketInTxHistory 0x205f5ab54.
use rustpush::vcrc::tx_history::{fec_unpack_frame_loss, TxHistory};
use rustpush::vcrc::PacketLossInput;

fn ts(t: f64) -> u16 { ((t * 1024.0) as u64 & 0xffff) as u16 }

#[test]
fn unpack_frame_loss_layouts() {
    assert_eq!(fec_unpack_frame_loss((10 << 6) | 3, 6), Some((10, 3)));
    assert_eq!(fec_unpack_frame_loss((200 << 8) | 7, 8), Some((200, 7)));
    assert_eq!(fec_unpack_frame_loss(5, 7), None);
}

#[test]
fn add_info_and_counters() {
    let mut h = TxHistory::default();
    h.track_sent_packet(0, false, true, 1, 100, false); // audio, counted (+0xe3 set)
    h.track_sent_packet(0, false, false, 1, 100, false); // audio, not counted
    h.track_sent_packet(1, false, false, 2, 1000, true); // video ×2, IPv6 headers
    h.track_sent_packet(1, true, false, 1, 1000, false); // video, excluded (+0xe2 set)
    assert_eq!((h.sent_audio_packets, h.sent_video_packets), (1, 2));
    assert_eq!(h.sent_bytes, 128 + 128 + 2 * 1048 + 1028);
    h.add_info(77, 1.5);
    assert_eq!((h.write_index, h.count), (1, 1));
    let r = h.records[0];
    assert_eq!((r.tx_timestamp, r.sent_audio, r.sent_video, r.time), (77, 1, 2, 1.5));
}

#[test]
fn unknown_echo_reports_nothing_but_zero_video_loss() {
    let mut h = TxHistory::default();
    h.add_info(100, 1.0);
    let out = h.compute_packet_loss(&PacketLossInput { echoed_timestamp: 5, ..Default::default() }, false);
    assert_eq!(out.most_burst_loss, Some(0));
    assert_eq!(out.packet_loss_rate_video, Some(0.0));
    assert_eq!(out.packet_loss_rate, None);
    assert_eq!(out.packet_loss_rate_audio, None);
}

#[test]
fn ten_percent_loss_over_ten_seconds() {
    // Every 20 ms: one audio and two video packets, one tx-history entry. Every 100 ms the
    // receiver echoes the newest entry with 90% of the audio packets and a frame-loss word of
    // 5 frames, 1 lost per 100 ms (the loss-stats layout).
    let mut h = TxHistory::default();
    let mut out = Default::default();
    let mut recv_audio = 0f64;
    for k in 1..=500u32 {
        let t = k as f64 * 0.02;
        h.track_sent_packet(0, false, true, 1, 160, false);
        h.track_sent_packet(1, false, false, 2, 1200, false);
        h.add_info(ts(t), t);
        recv_audio += 0.9;
        if k % 5 == 0 {
            let input = PacketLossInput {
                echoed_timestamp: ts(t),
                audio_received_packets: recv_audio as u32,
                video_received_packets: (5 << 6) | 1,
                total_received_kbytes: (k * 2) & 0xffff,
                packet_burst_loss: 1,
                ..Default::default()
            };
            out = h.compute_packet_loss(&input, false);
        }
    }
    let plr_1s = out.packet_loss_rate_audio.expect("1 s window");
    let plr_5s = out.packet_loss_rate.expect("5 s window");
    assert!((plr_1s - 0.1).abs() < 0.02, "1 s PLR {plr_1s}");
    assert!((plr_5s - 0.1).abs() < 0.02, "5 s PLR {plr_5s}");
    assert!((out.packet_loss_rate_video.unwrap() - 0.2).abs() < 1e-9);
    assert_eq!(out.most_burst_loss, Some(1));
}
