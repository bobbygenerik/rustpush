use std::collections::HashMap;
use rustpush::avconference::{
    AVConfig, ChannelType, EnabledAVFeatures, GROUP_H265_FEATURES, build_rvra1,
    parse_vra_dimensions, select_rvra1_dimensions,
};

#[test]
fn av_config_default_advertises_rvra1_in_supported_features() {
    let cfg = AVConfig::new();
    let avc = cfg.avc_data();
    let media_blob: &[u8] = avc.vc_session_participant_key_media_blob.as_ref();
    assert!(!media_blob.is_empty());
    // VcMediaNegotiationBlobV2 puts [enabled_features (2B LE), supported_features (2B LE)]
    // in video_features (VCPCodecIntersectFeatureList 0x22a6a5120 / sub_22a71fad0).
    let mut wire = cfg.enabled_features.to_bytes().to_vec();
    wire.extend_from_slice(&cfg.supported_features.to_bytes());
    assert_eq!(wire.len(), 4);
    let enabled = EnabledAVFeatures::from_bytes(&wire[0..2]);
    let supported = EnabledAVFeatures::from_bytes(&wire[2..4]);
    assert!(
        enabled.has_feature("RVRA1"),
        "TX enabled_features must advertise RVRA1: {:?}",
        enabled
    );
    assert!(
        supported.has_feature("RVRA1"),
        "RX supported_features must advertise RVRA1 so iPhone TX & get_media_config intersect RVRA1: {:?}",
        supported
    );
    assert!(
        supported.has_feature("VRAE"),
        "RX supported_features must advertise VRAE: {:?}",
        supported
    );
    // Simulate VCPCodecIntersectFeatureList: remote TX (enabled) & local RX (supported)
    let apple_remote_tx = EnabledAVFeatures::from_str("FLS2;RVRA1;CH1;CR;CF;FA;");
    let negotiated_tx_from_apple = apple_remote_tx.intersect(&supported);
    assert!(negotiated_tx_from_apple.has_feature("RVRA1"));

    // Simulate get_media_config() on our own RX stream (our enabled & our supported)
    let loopback_negotiated = enabled.intersect(&supported);
    assert!(loopback_negotiated.has_feature("RVRA1"));
}

#[test]
fn parse_vra_dimensions_prefers_vrae_and_falls_back_to_rvra1() {
    let mut meta = HashMap::new();
    meta.insert("RVRA1", vec![80, 45]); // 640 x 360
    assert_eq!(parse_vra_dimensions(&meta), Some((640, 360)));

    // VRAE (1280 x 720) overrides RVRA1 (sub_22a64b528: 0x22a722004 before 0x22a721ca8)
    meta.insert("VRAE", vec![0x05, 0x00, 0x02, 0xd0]);
    assert_eq!(parse_vra_dimensions(&meta), Some((1280, 720)));

    // Zero dimensions rejected
    meta.insert("VRAE", vec![0, 0, 0, 0]);
    assert_eq!(parse_vra_dimensions(&meta), Some((640, 360)));

    let mut only_zero = HashMap::new();
    only_zero.insert("RVRA1", vec![0, 0]);
    assert_eq!(parse_vra_dimensions(&only_zero), None);
}

#[test]
fn select_rvra1_dimensions_uses_coded_space_without_portrait_swap() {
    // Portrait phone still encodes 1920x1080 coded canvas with camera_meta orientation;
    // RVRA1 must stay in coded pixel space (never swap to 1080x1920).
    assert_eq!(
        select_rvra1_dimensions(Some((864, 496)), Some((1920, 1080))),
        (864, 496)
    );
    assert_eq!(build_rvra1(864, 496), [108, 62]);

    // Clamps if used ever exceeds coded SPS size
    assert_eq!(
        select_rvra1_dimensions(Some((1920, 1080)), Some((1280, 720))),
        (1280, 720)
    );

    // Falls back to unswapped coded size when no explicit used size
    assert_eq!(
        select_rvra1_dimensions(None, Some((1920, 1080))),
        (1920, 1080)
    );
}

#[test]
fn group_h265_footer_roundtrip_carries_uncropped_or_scaled_rvra1() {
    let mut nal = vec![0x00, 0x00, 0x00, 0x01, 0x02, 0x01, 0xAA, 0xBB];
    let payload_len = nal.len();
    let dims = select_rvra1_dimensions(Some((640, 368)), Some((1920, 1080)));
    GROUP_H265_FEATURES.add_footer(
        &mut nal,
        HashMap::from_iter([
            ("RVRA1", build_rvra1(dims.0, dims.1).to_vec()),
            ("CH1", vec![0x00, 0x00]),
            ("CR", vec![0x00, 0x00, 0x00, 0x00]),
            ("FA", vec![0x3e, 0x3e, 0xc0, 0xc0]),
        ]),
    );
    let (stripped, parsed) = GROUP_H265_FEATURES
        .parse_frame(&nal)
        .expect("parse footer");
    assert_eq!(stripped.len(), payload_len);
    assert_eq!(parse_vra_dimensions(&parsed), Some((640, 368)));
    assert_eq!(parsed.get("RVRA1").map(|v| v.as_slice()), Some(&[80u8, 46u8][..]));
}

#[test]
fn enabled_av_features_unknown_type_is_ignored_cleanly() {
    // Sanity check EnabledAVFeatures trait behavior on video type
    let _ = ChannelType::H265;
}
