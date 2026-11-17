use rustpush::vcrc_config::{mode1_camera_defaults, EFFECTIVE_VALUE_UNKNOWNS, TIER_BITRATES};

#[test]
fn binary_tier_table_spot_values_and_order() {
    // 24A446 AVConference __const, 0x2066a3b10 + index*4.
    assert_eq!(TIER_BITRATES.len(), 66);
    assert_eq!(TIER_BITRATES[17], 502000);
    assert_eq!(TIER_BITRATES[26], 1949000);
    assert_eq!(TIER_BITRATES[31], 4084000);
    assert!(TIER_BITRATES.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn cold_profile_tiers_follow_cap_and_zero_minimum() {
    let config = mode1_camera_defaults();
    // ConfigureInternal 0x205eaa5b0..5d0 selects max at config+0x14.
    assert_eq!(config.o_14, 31);
    assert!(TIER_BITRATES[config.o_14 as usize] <= 4084000);
    assert!(TIER_BITRATES[config.o_14 as usize + 1] > 4084000);
    // 0x205eb0708 supplies min=0; 0x205eaa5d4..5f8 selects config+0x18.
    assert_eq!(config.o_18, 0);
    // 0x205eaa5fc..614 computes cold 17; 0x205eaa890 stores config+0x10.
    // A live learnt-bitrate delegate is UNKNOWN and can replace this default.
    assert_eq!(config.o_10, 17);
    assert_eq!(config.o_8[config.o_10 as usize], 502000);
    assert!(EFFECTIVE_VALUE_UNKNOWNS.contains(&0x10));
}

#[test]
fn packed_constants_keep_integer_word_boundaries() {
    let config = mode1_camera_defaults();
    // 0x2066a3900 and 0x2066a3920 are transported through d registers.
    assert_eq!((config.o_1e8, config.o_1ec), (4, 3));
    assert_eq!((config.o_348, config.o_34c), (1200000, 600000));
}

#[test]
fn copied_thresholds_keep_exact_native_bits() {
    let config = mode1_camera_defaults();
    // PopulateCongestionLevelThresholds 0x2060de4d0; copy 0x205ead24c..270.
    assert_eq!(config.o_3a0.to_bits(), 0xbfecccccc0000000);
    assert_eq!(config.o_3a8.to_bits(), 0x400cccccc0000000);
    assert_eq!(config.o_3b0.to_bits(), 0x4009333328000000);
    assert_eq!(config.o_3b8.to_bits(), 0x4005999990000000);
    assert_eq!(config.o_3c0.to_bits(), 0x4001fffff8000000);
}
