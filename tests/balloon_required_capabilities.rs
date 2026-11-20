use std::io::Read;

use plist::Value;
use rustpush::{Balloon, ExtensionApp, KeyedArchive};

fn ungzip(bytes: &[u8]) -> Vec<u8> {
    let mut out = vec![];
    libflate::gzip::Decoder::new(bytes)
        .unwrap()
        .read_to_end(&mut out)
        .unwrap();
    out
}

fn app(bundle: &str) -> ExtensionApp {
    ExtensionApp {
        name: "Polls".to_string(),
        app_id: None,
        bundle_id: bundle.to_string(),
        balloon: Some(Balloon {
            url: "data:,e30=?src=p&c=1".to_string(),
            session: None,
            layout: None,
            ld_text: Some("Sent a poll".to_string()),
            is_live: true,
            icon: None,
        }),
    }
}

fn payload(app: &ExtensionApp) -> plist::Dictionary {
    let (_, balloon) = app.to_raw(false).unwrap();
    let root = KeyedArchive::expand_root(&ungzip(&balloon.unwrap())).unwrap();
    root.into_dictionary().unwrap()
}

fn data_bytes(v: &Value) -> Vec<u8> {
    match v {
        Value::Data(d) => d.clone(),
        Value::Dictionary(d) => match d.get("NS.data") {
            Some(Value::Data(d)) => d.clone(),
            other => panic!("unexpected NSData shape {other:?}"),
        },
        other => panic!("unexpected requiredCapabilities value {other:?}"),
    }
}

// MessagesPolls msMessageFrom (0x398dc..0x3990c) sets requiredCapabilities to
// [supports-polls]; IMSharedUtilities _IMBalloonPluginRequiredCapabilitiesWithPayloadDictionary
// (0x1a41dda90) reads payload["requiredCapabilities"] as keyed-archived NSArray<NSString>.
#[test]
fn polls_payload_carries_archived_required_capabilities() {
    let dict = payload(&app(
        "com.apple.messages.MSMessageExtensionBalloonPlugin:0000000000:com.apple.messages.Polls",
    ));
    let caps = dict.get("requiredCapabilities").expect("requiredCapabilities present");
    let root = KeyedArchive::expand_root(&data_bytes(caps)).unwrap();
    let objects: Vec<String> = match root {
        Value::Dictionary(d) => plist::from_value(d.get("NS.objects").unwrap()).unwrap(),
        Value::Array(a) => plist::from_value(&Value::Array(a)).unwrap(),
        other => panic!("unexpected archive root {other:?}"),
    };
    assert_eq!(objects, vec!["supports-polls".to_string()]);
}

#[test]
fn other_balloons_have_no_required_capabilities() {
    let dict = payload(&app("com.apple.messages.URLBalloonProvider"));
    assert!(dict.get("requiredCapabilities").is_none());
}
