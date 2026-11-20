use rustpush::{Attachment, AttachmentType, MessagePart, MessageParts};

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

#[test]
fn file_filename_escapes_xml_metacharacters_and_absent_name_is_skipped() {
    // iMessage.imservice 0x44328 calls IMCreateEscapedAttributeValueFromString.
    // The Apple reader at 0x423a0 skips a missing name.
    let parts = MessageParts::parse_parts_xml(
        r#"<html><body><FILE datasize="3"/><FILE name="A&amp;B &quot;x&quot; &lt;&apos;y&apos;&gt;.jpg" datasize="1200"/></body></html>"#,
    );
    assert_eq!(parts.0.len(), 1);
    let MessagePart::Attachment(attachment) = &parts.0[0].part else {
        panic!("expected attachment");
    };
    assert_eq!(attachment.name, "A&B \"x\" <'y'>.jpg");
    assert!(parts.to_xml_string().contains(r#"name="A&amp;B &quot;x&quot; &lt;&apos;y&apos;&gt;.jpg""#));
}

#[test]
fn file_companion_marker_uses_nonempty_receive_and_canonical_send_values() {
    // iMessage.imservice 0x42430/0x4243c reads the iris string and tests length.
    // The serializer at 0x44bac/0x44bc8 emits the companion value "yes".
    let parts = MessageParts::parse_parts_xml(
        r#"<html><body><FILE name="photo.jpg" iris="" datasize="2"/><FILE name="photo.mov" iris="1" datasize="3"/></body></html>"#,
    );
    let [photo, companion] = parts.0.as_slice() else {
        panic!("expected photo and companion");
    };
    let MessagePart::Attachment(photo) = &photo.part else {
        panic!("expected photo attachment");
    };
    let MessagePart::Attachment(companion) = &companion.part else {
        panic!("expected companion attachment");
    };
    assert!(!photo.iris);
    assert!(companion.iris);
    let xml = parts.to_xml_string();
    assert_eq!(xml.matches("iris=").count(), 1);
    assert!(xml.contains(r#"datasize="3" iris="yes""#));
}

#[test]
fn file_reader_resolves_inline_refs_as_plist_data_keys() {
    let payload = plist::Dictionary::from_iter([
        ("ia-0", plist::Value::String("wrong type".into())),
        ("ia-2", plist::Value::Data(vec![1, 2, 3])),
        ("ia-4", plist::Value::Data(vec![4])),
    ]);
    // iMessage.imservice 0x6efe8 uses the key from 0x6ef80 directly.
    let parts = MessageParts::parse_parts(
        r#"<html><body>
          <FILE name="bad type" inline-attachment="ia-0"/>
          <FILE name="second" inline-attachment="ia-2" width="44" height="55"/>
          <FILE name="missing value" inline-attachment="ia-3"/>
          <FILE name="fourth" inline-attachment="ia-4"/>
        </body></html>"#,
        Some(&payload),
    );
    let [second, fourth] = parts.0.as_slice() else {
        panic!("expected both resolvable attachments");
    };
    let MessagePart::Attachment(second) = &second.part else {
        panic!("expected second attachment");
    };
    let MessagePart::Attachment(fourth) = &fourth.part else {
        panic!("expected fourth attachment");
    };
    assert!(matches!(&second.a_type, AttachmentType::Inline(bytes) if bytes == &[1, 2, 3]));
    assert_eq!((second.width, second.height), (Some(44), Some(55)));
    assert!(matches!(&fourth.a_type, AttachmentType::Inline(bytes) if bytes == &[4]));
}
