use rustpush::{Attachment, MessagePart, MessageParts};

#[test]
fn file_reader_accepts_signed_decimal_dimensions_and_preserves_missing_values() {
    // iMessage.imservice 0x42354: _stringForKey: followed by integerValue, then
    // appendInlineImageWithGUID:filename:width:height:isAnimoji:isAdaptiveImageGlyph:.
    let parts = MessageParts::parse_parts_xml(
        r#"<html><body><FILE name="a" width="-1" height="768" datasize="1200"/>
        <FILE name="b" height="invalid" datasize="1200"/></body></html>"#,
    );
    let [first, second] = parts.0.as_slice() else {
        panic!("expected two attachments");
    };
    let MessagePart::Attachment(first) = &first.part else {
        panic!("expected attachment");
    };
    let MessagePart::Attachment(second) = &second.part else {
        panic!("expected attachment");
    };
    assert_eq!((first.width, first.height), (Some(-1), Some(768)));
    assert_eq!((second.width, second.height), (None, None));
    assert_eq!(first.get_size(), 1200);

    let xml = parts.to_xml_string();
    assert!(xml.contains(r#"width="-1" height="768" datasize="1200""#));
    assert!(xml.contains(r#"width="0" height="0" datasize="1200""#));
}

#[test]
fn persisted_attachments_from_before_dimension_support_still_decode() {
    let parts = MessageParts::parse_parts_xml(r#"<FILE name="legacy" datasize="1200"/>"#);
    let MessagePart::Attachment(attachment) = &parts.0[0].part else {
        panic!("expected attachment");
    };
    let value = plist::to_value(attachment).unwrap();
    let fields = value.as_dictionary().unwrap();
    assert!(fields.get("width").is_none());
    assert!(fields.get("height").is_none());
    let restored: Attachment = plist::from_value(&value).unwrap();
    assert_eq!((restored.width, restored.height), (None, None));
    assert_eq!(restored.get_size(), 1200);
}
