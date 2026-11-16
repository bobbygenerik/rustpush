//! Integration tests for 1:1 outgoing OWRD computation and wire packing.

use rustpush::avconference::receive_clock::{pack_q13_delay, AudioOwrdTracker};
use rustpush::jlag_processor::{JLagProcessor, LagPacket};

#[test]
fn nan_before_first_record_packs_to_0xffff() {
    assert_eq!(pack_q13_delay(f64::NAN), 0xffff);

    let mut tracker = AudioOwrdTracker::new(24_000, 480);
    assert!(tracker.owrd_or_nan().is_nan());
    assert_eq!(pack_q13_delay(tracker.owrd_or_nan()), 0xffff);

    // First audio packet runs NewStream + Process(state = 1): RecordOWRD has not run yet.
    let raw_after_p0 = tracker.observe_packet(1, 480, 0.0, false);
    assert!(raw_after_p0.is_nan());
    assert_eq!(pack_q13_delay(raw_after_p0), 0xffff);
}

#[test]
fn steady_30ms_extra_delay_sequence_matches_kotlin_reference_exact() {
    // Inputs & exact expected outputs captured by running FaceTimeJLagProcessor.kt on:
    //   tsRate = 24000, samplesPerFrame = 480
    //   p0: seq=1, ts=480, arrival=0.0 (newStream + process state=1)
    //   i=1..25: seq=1+i, ts=480+i*480, arrival=i*0.020+0.030 (process state=5)
    let kotlin_expected: &[(u32, u16)] = &[
        (994339153, 24),   // i=1  owrd_f32=0.002997
        (1002083517, 46),  // i=2  owrd_f32=0.0056940005
        (1006964210, 66),  // i=3  owrd_f32=0.008121001
        (1009309263, 84),  // i=4  owrd_f32=0.010305001
        (1011419490, 100), // i=5  owrd_f32=0.012270303
        (1013318371, 115), // i=6  owrd_f32=0.014038774
        (1015024306, 128), // i=7  owrd_f32=0.0156301
        (1015793047, 139), // i=8  owrd_f32=0.017061992
        (1016484754, 150), // i=9  owrd_f32=0.018350396
        (1017107129, 159), // i=10 owrd_f32=0.01950966
        (1017667106, 168), // i=11 owrd_f32=0.020552699
        (1018170925, 176), // i=12 owrd_f32=0.021491135
        (1018624200, 182), // i=13 owrd_f32=0.022335425
        (1019031988, 189), // i=14 owrd_f32=0.02309499
        (1019398836, 194), // i=15 owrd_f32=0.023778297
        (1019728839, 199), // i=16 owrd_f32=0.024392975
        (1020025681, 204), // i=17 owrd_f32=0.024945887
        (1020292678, 208), // i=18 owrd_f32=0.025443207
        (1020532814, 212), // i=19 owrd_f32=0.025890496
        (1020748777, 215), // i=20 owrd_f32=0.026292758
        (1020942982, 218), // i=21 owrd_f32=0.026654493
        (1021117607, 221), // i=22 owrd_f32=0.026979757
        (1021274608, 223), // i=23 owrd_f32=0.027272195
        (1021415749, 225), // i=24 owrd_f32=0.02753509
        (1021542615, 227), // i=25 owrd_f32=0.027771397
    ];

    let mut tracker = AudioOwrdTracker::new(24_000, 480);
    let raw0 = tracker.observe_packet(1, 480, 0.0, false);
    assert!(raw0.is_nan());
    assert_eq!(pack_q13_delay(raw0), 0xffff);

    for (idx, &(expected_f32_bits, expected_packed)) in kotlin_expected.iter().enumerate() {
        let i = idx + 1;
        let seq = (1 + i) as u16;
        let ts = (480 + i * 480) as u32;
        let arrival = i as f64 * 0.020 + 0.030;
        let raw = tracker.observe_packet(seq, ts, arrival, false);
        assert_eq!(
            tracker.processor().get_owrd().to_bits(),
            expected_f32_bits,
            "raw f32 bits mismatch at step i={i}"
        );
        assert_eq!(
            pack_q13_delay(raw),
            expected_packed,
            "packed Q13 mismatch at step i={i}"
        );
    }
}

#[test]
fn outlier_6s_is_rejected() {
    // 1) Direct RecordOWRD rejection when (lag - owrd_long) > 5.0 (0x2064a6d94):
    let mut proc = JLagProcessor::new();
    let p0 = LagPacket {
        sequence_number: 1,
        timestamp: 480,
        arrival_sec: 0.0,
        field18: 480,
        field20: 1,
        ..Default::default()
    };
    proc.new_stream(Some(&p0), 24_000, 480);
    proc.packet_counter = 5;
    proc.owrd_short = 0.010;
    proc.owrd_long = 0.020;
    proc.owrd = 0.010;
    proc.record_owrd(6.0);
    assert_eq!(proc.owrd_short, 0.010);
    assert_eq!(proc.owrd_long, 0.020);
    assert_eq!(proc.get_owrd(), 0.010);

    // 2) Full tracker path: steady baseline followed by a 6.0 s arrival jump does not corrupt OWRD:
    let mut tracker = AudioOwrdTracker::new(24_000, 480);
    assert!(tracker.observe_packet(1, 480, 0.0, false).is_nan());
    for i in 2..=6 {
        let owrd = tracker.observe_packet(i, u32::from(i) * 480, f64::from(i - 1) * 0.020, false);
        assert_eq!(pack_q13_delay(owrd), 0);
    }
    let after_outlier = tracker.observe_packet(7, 7 * 480, 6.0 + 6.0 * 0.020, false);
    assert_eq!(after_outlier, 0.0);
    assert_eq!(pack_q13_delay(after_outlier), 0);
}

#[test]
fn negative_difference_clamps_as_apple_does() {
    // Direct RecordOWRD (0x2064a6eb8..0x2064a6ed0): when owrd_long - owrd_short < 0.0,
    // owrd_short is pulled down to owrd_long and owrd clamps to 0.0.
    let mut proc = JLagProcessor::new();
    let p0 = LagPacket {
        sequence_number: 1,
        timestamp: 480,
        arrival_sec: 0.0,
        field18: 480,
        field20: 1,
        ..Default::default()
    };
    proc.new_stream(Some(&p0), 24_000, 480);
    proc.packet_counter = 5;
    proc.owrd_short = 0.020;
    proc.owrd_long = 0.010;
    proc.record_owrd(0.010);
    let expected_long = 0.010_f64 * 0.9 + 0.010_f64 * 0.1;
    assert_eq!(proc.owrd_long, expected_long);
    assert_eq!(proc.owrd_short, proc.owrd_long);
    assert_eq!(proc.get_owrd(), 0.0);
    assert_eq!(pack_q13_delay(f64::from(proc.get_owrd())), 0);

    // Negative packing also clamps to 0 (0x205fa57d4):
    assert_eq!(pack_q13_delay(-0.001), 0);
    assert_eq!(pack_q13_delay(-5.0), 0);
}

#[test]
fn values_at_or_above_0xffff_saturate_to_0xffff() {
    assert_eq!(pack_q13_delay(7.9999), 0xffff);
    assert_eq!(pack_q13_delay(8.0), 0xffff);
    assert_eq!(pack_q13_delay(100.0), 0xffff);
    assert_eq!(pack_q13_delay(f64::INFINITY), 0xffff);
}
