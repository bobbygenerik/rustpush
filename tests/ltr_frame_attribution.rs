#[path = "../src/video_frame_control.rs"]
mod video_frame_control;
#[path = "../src/video_reference_chain.rs"]
mod video_reference_chain;

use video_frame_control::{VideoFrameControlTracker, MAX_TRACKED_FRAMES};
use video_reference_chain::VideoReferenceChain;

/// Hand-derived from `_VideoPacketBuffer_AddPacket` (`0x205fd55e8`):
/// - New frame allocation (`0x205fd6404..408`): stores first arriving packet's type-4 LTR flags
///   into `frame + 0x20`; `loc_205fd5f38` never overwrites or ORs `[x22, #0x20]`.
/// - Camera status (`frame + 0x24`, `0x205fd640c..410` and `0x205fd5f40..f4c`): keeps first non-zero.
#[test]
fn refresh_frame_followed_by_normal_frame_preserves_per_frame_labels() {
    let mut tracker = VideoFrameControlTracker::<u8>::default();
    let ssrc = 0x1020_3040;

    // Frame 1000: refresh frame (`ltr_bits = 0x02`) across 2 packets, then frame 2000: normal (`0x00`).
    tracker.record(ssrc, 1000, 0x02, Some(777), None);
    tracker.record(ssrc, 1000, 0x05, Some(888), Some(0x83));
    tracker.record(ssrc, 2000, 0x00, None, Some(0x81));

    let refresh = tracker.take(ssrc, 1000).expect("frame 1000 present");
    assert_eq!(refresh.ltr_bits, 0x02);
    assert_eq!(refresh.ltr_timestamp, Some(777));
    assert_eq!(refresh.camera_meta, Some(0x83));

    let normal = tracker.take(ssrc, 2000).expect("frame 2000 present");
    assert_eq!(normal.ltr_bits, 0x00);
    assert_eq!(normal.ltr_timestamp, None);
    assert_eq!(normal.camera_meta, Some(0x81));

    // Verify with VideoReferenceChain after loss: frame 1000 clears wait cleanly; frame 2000 is admitted.
    let mut chain = VideoReferenceChain::default();
    assert!(chain.accept_with_ltr((1, 0), false, true, 0x00, 0));
    assert!(!chain.accept_with_ltr((1, 0), false, false, 0x00, 2));
    assert!(chain.accept_with_ltr((1, 0), false, false, refresh.ltr_bits, 0));
    assert!(chain.accept_with_ltr((1, 0), false, false, normal.ltr_bits, 0));
}

#[test]
fn late_popped_frame_keeps_its_own_control_info_after_later_packets() {
    let mut tracker = VideoFrameControlTracker::<u8>::default();
    let ssrc = 0xdead_beef;

    // Frame 100 (LTR refresh + ACK candidate: 0x03) arrives, then packets for frames 200 & 300 arrive
    // before SampleBuilder pops frame 100.
    tracker.record(ssrc, 100, 0x03, Some(0x1111_2222), Some(0x82));
    tracker.record(ssrc, 200, 0x00, None, Some(0x84));
    tracker.record(ssrc, 300, 0x01, Some(0x3333_4444), Some(0x85));

    let f100 = tracker.take(ssrc, 100).expect("frame 100 popped late");
    assert_eq!(f100.ltr_bits, 0x03);
    assert_eq!(f100.ltr_timestamp, Some(0x1111_2222));
    assert_eq!(f100.camera_meta, Some(0x82));

    let f200 = tracker.take(ssrc, 200).expect("frame 200");
    assert_eq!(f200.ltr_bits, 0x00);
    assert_eq!(f200.ltr_timestamp, None);
    assert_eq!(f200.camera_meta, Some(0x84));

    let f300 = tracker.take(ssrc, 300).expect("frame 300");
    assert_eq!(f300.ltr_bits, 0x01);
    assert_eq!(f300.ltr_timestamp, Some(0x3333_4444));
    assert_eq!(f300.camera_meta, Some(0x85));
}

#[test]
fn rtp_timestamp_wraparound_evicts_older_and_preserves_newer() {
    let mut tracker = VideoFrameControlTracker::<u8>::default();
    let ssrc = 42;
    let other_ssrc = 99;

    let ts_old = u32::MAX - 10;
    let ts_pop = 5;
    let ts_new = 20;

    tracker.record(ssrc, ts_old, 0x01, Some(10), Some(0x80));
    tracker.record(ssrc, ts_pop, 0x02, Some(20), Some(0x81));
    tracker.record(ssrc, ts_new, 0x00, Some(30), Some(0x82));
    tracker.record(other_ssrc, ts_old, 0x03, Some(40), Some(0x83));

    // Popping ts_pop = 5 must evict ts_old = u32::MAX - 10 for ssrc, preserve ts_new = 20,
    // and leave other_ssrc untouched.
    let popped = tracker.take(ssrc, ts_pop).expect("wrapped frame 5");
    assert_eq!(popped.ltr_bits, 0x02);
    assert!(tracker.take(ssrc, ts_old).is_none());

    let newer = tracker.take(ssrc, ts_new).expect("newer frame 20");
    assert_eq!(newer.ltr_bits, 0x00);
    assert_eq!(newer.ltr_timestamp, Some(30));

    let unrelated = tracker.take(other_ssrc, ts_old).expect("other ssrc preserved");
    assert_eq!(unrelated.ltr_bits, 0x03);
}

#[test]
fn attribution_map_stays_bounded_without_pops() {
    let mut tracker = VideoFrameControlTracker::<u8>::default();
    let ssrc = 7;

    for i in 0..(MAX_TRACKED_FRAMES as u32 + 128) {
        tracker.record(ssrc, i * 3000, (i & 0xf) as u8, Some(i), Some(0x80));
        assert!(tracker.len() <= MAX_TRACKED_FRAMES);
    }
    assert_eq!(tracker.len(), MAX_TRACKED_FRAMES);

    // Popping near the end evicts all older entries for that SSRC.
    let latest_ts = (MAX_TRACKED_FRAMES as u32 + 127) * 3000;
    assert!(tracker.take(ssrc, latest_ts).is_some());
    assert_eq!(tracker.len(), 0);
}
