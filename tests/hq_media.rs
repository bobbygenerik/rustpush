use std::collections::HashMap;

use openssl::ec::{EcGroup, EcKey};
use openssl::nid::Nid;
use openssl::rsa::Rsa;
use plist::Value;
use rustpush::hq_media::{
    MESSAGE_SENDABLE_UTIS, MMCS_HQ_PHOTO_REPORTED_SIZE_OVERRIDE,
    MMCS_HQ_VIDEO_REPORTED_SIZE_OVERRIDE, MODERN_HIGH_QUALITY_PHOTO_SIZE_LIMIT,
    MODERN_HIGH_QUALITY_VIDEO_SIZE_LIMIT, SMALLER_IMAGE_FILE_SIZE,
    STANDARD_PHOTO_CELLULAR_SIZE_LIMIT, STANDARD_PHOTO_WIFI_SIZE_LIMIT,
    STANDARD_VIDEO_CELLULAR_SIZE_LIMIT, STANDARD_VIDEO_WIFI_SIZE_LIMIT,
    TINY_IMAGE_QUALITY_SIZE_LIMIT, compute_common_hq_capabilities,
    mmcs_target_report_size_for_hq_photo, mmcs_target_report_size_for_hq_video,
};
use rustpush::{IDSPublicIdentity, MADRID_SERVICE, ParsedClientData};

fn dummy_public_identity() -> IDSPublicIdentity {
    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
    let ec_priv = EcKey::generate(&group).unwrap();
    let ec_pub = EcKey::from_public_key(&group, ec_priv.public_key()).unwrap();
    let rsa_priv = Rsa::generate(1280).unwrap();
    let rsa_pub = Rsa::from_public_components(
        rsa_priv.n().to_owned().unwrap(),
        rsa_priv.e().to_owned().unwrap(),
    )
    .unwrap();
    IDSPublicIdentity {
        signing_key: ec_pub,
        encryption_key: rsa_pub,
    }
}

fn sample_client_data(
    ident: &IDSPublicIdentity,
    hq_video: Option<Value>,
    hq_photo: Option<Value>,
) -> ParsedClientData {
    let mut dict = plist::Dictionary::new();
    dict.insert(
        "public-message-identity-key".to_string(),
        plist::to_value(ident).expect("serialize IDSPublicIdentity"),
    );
    if let Some(v) = hq_video {
        dict.insert("supports-high-quality-video".to_string(), v);
    }
    if let Some(p) = hq_photo {
        dict.insert("supports-high-quality-photo-file-sizes".to_string(), p);
    }
    plist::from_value(&Value::Dictionary(dict)).expect("valid ParsedClientData plist")
}

#[test]
fn malformed_certified_delivery_capability_does_not_fail_peer_lookup() {
    let identity = plist::to_value(&dummy_public_identity()).unwrap();
    let key = "supports-certified-delivery-v1";
    let cases = [
        (None, false),
        (Some(Value::Boolean(true)), true),
        (Some(Value::Integer(1.into())), true),
        (Some(Value::Integer(2.into())), false),
        (Some(Value::Real(1.0)), true),
        (Some(Value::String("YES".into())), true),
        (Some(Value::Dictionary(plist::Dictionary::new())), false),
        (Some(Value::Data(vec![1])), false),
    ];
    for (value, expected) in cases {
        let mut dict = plist::Dictionary::from_iter([("public-message-identity-key", identity.clone())]);
        if let Some(value) = value {
            dict.insert(key.to_owned(), value);
        }
        let parsed: ParsedClientData = plist::from_value(&Value::Dictionary(dict)).unwrap();
        assert_eq!(parsed.supports_certified_delivery_v1, expected);
    }
}

#[test]
fn madrid_service_advertises_hq_video_and_photo_capabilities() {
    let entries: HashMap<&str, &Value> = MADRID_SERVICE
        .client_data
        .iter()
        .map(|(k, v)| (*k, v))
        .collect();
    assert_eq!(
        entries.get("supports-high-quality-video").copied(),
        Some(&Value::Boolean(true)),
        "IDSFoundation _IDSRegistrationPropertySupportsHighQualityVideo (0x1e0c055c8) missing"
    );
    assert_eq!(
        entries
            .get("supports-high-quality-photo-file-sizes")
            .copied(),
        Some(&Value::Boolean(true)),
        "IDSFoundation _IDSRegistrationPropertySupportsHighQualityPhotoFileSizes (0x1e0c05540) missing"
    );
}

#[test]
fn parsed_client_data_decodes_bool_and_integer_hq_flags() {
    let ident = dummy_public_identity();
    let bool_true = sample_client_data(&ident, Some(Value::Boolean(true)), Some(Value::Boolean(true)));
    assert!(bool_true.supports_high_quality_video);
    assert!(bool_true.supports_high_quality_photo_file_sizes);

    let int_one = sample_client_data(
        &ident,
        Some(Value::Integer(1.into())),
        Some(Value::Integer(1.into())),
    );
    assert!(int_one.supports_high_quality_video);
    assert!(int_one.supports_high_quality_photo_file_sizes);

    let int_two = sample_client_data(
        &ident,
        Some(Value::Integer(2.into())),
        Some(Value::Integer(0.into())),
    );
    assert!(!int_two.supports_high_quality_video);
    assert!(!int_two.supports_high_quality_photo_file_sizes);

    // Build 1019 regression: an unexpected value type must not fail the whole lookup.
    let odd = sample_client_data(
        &ident,
        Some(Value::String("1".into())),
        Some(Value::Data(vec![1, 2, 3])),
    );
    assert!(odd.supports_high_quality_video);
    assert!(!odd.supports_high_quality_photo_file_sizes);
    let real = sample_client_data(&ident, Some(Value::Real(1.0)), Some(Value::Real(2.5)));
    assert!(real.supports_high_quality_video);
    assert!(!real.supports_high_quality_photo_file_sizes);

    let omitted = sample_client_data(&ident, None, None);
    assert!(!omitted.supports_high_quality_video);
    assert!(!omitted.supports_high_quality_photo_file_sizes);
}

#[test]
fn common_hq_capabilities_requires_all_participants_and_devices() {
    let empty: HashMap<String, Vec<ParsedClientData>> = HashMap::new();
    let caps_empty = compute_common_hq_capabilities(&empty);
    assert!(!caps_empty.supports_high_quality_video);
    assert!(!caps_empty.supports_high_quality_photo_file_sizes);

    let ident = dummy_public_identity();
    let mut all_hq = HashMap::new();
    all_hq.insert(
        "tel:+15551234567".to_string(),
        vec![sample_client_data(
            &ident,
            Some(Value::Boolean(true)),
            Some(Value::Integer(1.into())),
        )],
    );
    all_hq.insert(
        "mailto:friend@icloud.com".to_string(),
        vec![
            sample_client_data(&ident, Some(Value::Boolean(true)), Some(Value::Boolean(true))),
            sample_client_data(
                &ident,
                Some(Value::Integer(1.into())),
                Some(Value::Boolean(true)),
            ),
        ],
    );
    let caps_ok = compute_common_hq_capabilities(&all_hq);
    assert!(caps_ok.supports_high_quality_video);
    assert!(caps_ok.supports_high_quality_photo_file_sizes);

    // One legacy device among a recipient's endpoints disables HQ video
    let mut mixed_video = all_hq.clone();
    mixed_video
        .get_mut("mailto:friend@icloud.com")
        .unwrap()
        .push(sample_client_data(&ident, None, Some(Value::Boolean(true))));
    let caps_mixed = compute_common_hq_capabilities(&mixed_video);
    assert!(!caps_mixed.supports_high_quality_video);
    assert!(caps_mixed.supports_high_quality_photo_file_sizes);
}

#[test]
fn ipsw_size_limits_and_mmcs_report_size_overrides_match_24a446() {
    assert_eq!(MODERN_HIGH_QUALITY_VIDEO_SIZE_LIMIT, 0x640_0000);
    assert_eq!(MODERN_HIGH_QUALITY_PHOTO_SIZE_LIMIT, 0x640_0000);
    assert_eq!(STANDARD_VIDEO_WIFI_SIZE_LIMIT, 0xa0_0000);
    assert_eq!(STANDARD_VIDEO_CELLULAR_SIZE_LIMIT, 0x40_0000);
    assert_eq!(STANDARD_PHOTO_WIFI_SIZE_LIMIT, 0xa0_0000);
    assert_eq!(STANDARD_PHOTO_CELLULAR_SIZE_LIMIT, 0x40_0000);
    assert_eq!(SMALLER_IMAGE_FILE_SIZE, 0x7_d000);
    assert_eq!(TINY_IMAGE_QUALITY_SIZE_LIMIT, 0x5_7800);
    assert_eq!(MMCS_HQ_VIDEO_REPORTED_SIZE_OVERRIDE, 0x13f_ffff);
    assert_eq!(MMCS_HQ_PHOTO_REPORTED_SIZE_OVERRIDE, 0x10_0000);
    assert!(MESSAGE_SENDABLE_UTIS.contains(&"public.jpeg"));
    assert!(MESSAGE_SENDABLE_UTIS.contains(&"public.png"));
    assert!(!MESSAGE_SENDABLE_UTIS.contains(&"public.heic"));

    // IMDaemonCore mmcsTargetReportSizeForHighQualityVideoSize:commonCapabilities: (0x2346d1d4c)
    assert_eq!(
        mmcs_target_report_size_for_hq_video(45_000_000, false, true, true, None),
        20_971_519
    );
    assert_eq!(
        mmcs_target_report_size_for_hq_video(15_000_000, false, true, true, None),
        15_000_000
    );
    assert_eq!(
        mmcs_target_report_size_for_hq_video(45_000_000, true, true, true, None),
        45_000_000
    );
    assert_eq!(
        mmcs_target_report_size_for_hq_video(45_000_000, false, false, true, None),
        45_000_000
    );
    assert_eq!(
        mmcs_target_report_size_for_hq_video(45_000_000, false, true, false, None),
        45_000_000
    );
    assert_eq!(
        mmcs_target_report_size_for_hq_video(
            MODERN_HIGH_QUALITY_VIDEO_SIZE_LIMIT + 1,
            false,
            true,
            true,
            None
        ),
        MODERN_HIGH_QUALITY_VIDEO_SIZE_LIMIT + 1
    );

    // IMDaemonCore mmcsTargetReportSizeForHighQualityPhotoSize:commonCapabilities: (0x2346d1b68)
    assert_eq!(
        mmcs_target_report_size_for_hq_photo(8_000_000, false, true, true, None),
        1_048_576
    );
    assert_eq!(
        mmcs_target_report_size_for_hq_photo(8_000_000, true, true, true, None),
        8_000_000
    );

    // Feature B5: supports-xhtml-translation + tx XML encode/decode round-trip
    assert!(MADRID_SERVICE
        .client_data
        .iter()
        .any(|(k, v)| *k == "supports-xhtml-translation" && v.as_boolean() == Some(true)));
    let ident = dummy_public_identity();
    let mut dict = plist::Dictionary::new();
    dict.insert(
        "public-message-identity-key".to_string(),
        plist::to_value(&ident).expect("serialize IDSPublicIdentity"),
    );
    dict.insert("supports-xhtml-translation".to_string(), Value::Integer(1.into()));
    let client_data: ParsedClientData =
        plist::from_value(&Value::Dictionary(dict)).expect("valid ParsedClientData");
    let caps = compute_common_hq_capabilities(&HashMap::from([(
        "mailto:peer@example.com".to_string(),
        vec![client_data],
    )]));
    assert!(caps.supports_xhtml_translation);
    let tx = rustpush::encode_translated_html("Hola mundo", Some("en"), Some("es"));
    let decoded = rustpush::decode_translated_html(&tx).expect("tx xml decodes");
    assert_eq!(decoded.source_language.as_deref(), Some("en"));
    assert_eq!(decoded.translation_language.as_deref(), Some("es"));
    assert_eq!(decoded.text, "Hola mundo");
}

#[test]
fn lqm_hq_capability_requires_every_device() {
    let identity = plist::to_value(&dummy_public_identity()).unwrap();
    let dev = |lqm: Option<Value>| -> ParsedClientData {
        let mut dict = plist::Dictionary::from_iter([("public-message-identity-key", identity.clone())]);
        if let Some(v) = lqm {
            dict.insert("supports-lqm-hq".to_owned(), v);
        }
        plist::from_value(&Value::Dictionary(dict)).unwrap()
    };
    let mut all = HashMap::new();
    all.insert("tel:+1".to_string(), vec![dev(Some(Value::Boolean(true))), dev(Some(Value::Integer(1.into())))]);
    assert!(compute_common_hq_capabilities(&all).supports_lqm_hq);
    all.insert("tel:+2".to_string(), vec![dev(None)]);
    assert!(!compute_common_hq_capabilities(&all).supports_lqm_hq);
}
