use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::PathBuf,
    sync::{Mutex, atomic::{AtomicBool, Ordering}},
};

use plist::Value;
use serde_json::{json, Map, Value as Json};
use xml::{reader::XmlEvent, writer::{EmitterConfig, XmlEvent as WriteEvent}, EventReader};

use crate::ids::MessageBody;

const MADRID_TOPIC: &str = "com.apple.madrid";
// Bound diagnostic-only decompression of nested blobs. This does not limit delivery.
const DECODE_BUDGET: usize = 8 * 1024 * 1024;
static ENABLED: AtomicBool = AtomicBool::new(false);
static OUTPUT: Mutex<Option<File>> = Mutex::new(None);

pub fn set_directory(directory: Option<PathBuf>) -> io::Result<()> {
    let mut output = OUTPUT.lock().map_err(|_| io::Error::other("capture lock poisoned"))?;
    if let Some(directory) = directory {
        fs::create_dir_all(&directory)?;
        let path = directory.join(format!("madrid-{}.jsonl", uuid::Uuid::new_v4()));
        *output = Some(OpenOptions::new().write(true).create_new(true).open(path)?);
    } else {
        *output = None;
    }
    ENABLED.store(output.is_some(), Ordering::Release);
    Ok(())
}

pub fn enabled_for(topic: &str) -> bool {
    topic == MADRID_TOPIC && is_enabled()
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}

pub(crate) enum Body<'a> {
    Bytes(&'a [u8]),
    Plist(&'a Value),
}

impl<'a> From<&'a MessageBody> for Body<'a> {
    fn from(body: &'a MessageBody) -> Self {
        match body {
            MessageBody::Bytes(bytes) => Self::Bytes(bytes),
            MessageBody::Plist(value) => Self::Plist(value),
        }
    }
}

pub(crate) fn record_control_send(topic: &str, data: &impl serde::Serialize) {
    if enabled_for(topic) {
        if let Ok(envelope) = plist::to_value(data) {
            record(topic, "outbound", &envelope, None, None);
        }
    }
}

pub(crate) fn record(
    topic: &str, direction: &str, envelope: &Value, body: Option<Body<'_>>, recipient_index: Option<usize>,
) {
    if !enabled_for(topic) {
        return;
    }
    let Ok(mut output) = OUTPUT.lock() else { return };
    let Some(file) = output.as_mut() else { return };
    let result = (|| -> io::Result<()> {
        let mut line = serde_json::to_vec(&capture(direction, envelope, body, recipient_index))?;
        line.push(b'\n');
        file.write_all(&line)
    })();
    if let Err(error) = result {
        log::warn!("iMessage capture stopped after write failure: {error}");
        *output = None;
        ENABLED.store(false, Ordering::Release);
    }
}

fn capture(direction: &str, envelope: &Value, body: Option<Body<'_>>, recipient_index: Option<usize>) -> Json {
    let mut xhtml = Map::new();
    let body = match body {
        Some(Body::Bytes(bytes)) => capture_bytes(bytes, &mut xhtml),
        Some(Body::Plist(Value::Data(bytes))) => capture_bytes(bytes, &mut xhtml),
        Some(Body::Plist(value)) => plist_json(value, &mut Vec::new(), &mut xhtml),
        None => Json::Null,
    };
    let command = envelope.as_dictionary().and_then(|dict| dict.get("c")).and_then(Value::as_unsigned_integer);
    let mut clean_envelope = plist_json(envelope, &mut Vec::new(), &mut Map::new());
    if let (Some(150 | 151), Some(dict)) = (command, envelope.as_dictionary()) {
        for field in ["mR", "mU", "cH", "mS"] {
            if let Some(value) = dict.get(field) {
                clean_envelope[field] = redacted(value);
            }
        }
    }
    json!({
        "schema": 1,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "direction": direction,
        "command": command,
        "envelope": clean_envelope,
        "plist": body,
        "recipientIndex": recipient_index,
        "xhtml": xhtml,
    })
}

fn capture_bytes(bytes: &[u8], xhtml: &mut Map<String, Json>) -> Json {
    decode_plist(bytes).map(|value| plist_json(&value, &mut Vec::new(), xhtml))
        .unwrap_or_else(|| opaque_data(bytes))
}

fn decode_plist(bytes: &[u8]) -> Option<Value> {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut decoded = Vec::new();
        libflate::gzip::Decoder::new(bytes).ok()?
            .take((DECODE_BUDGET + 1) as u64).read_to_end(&mut decoded).ok()?;
        if decoded.len() > DECODE_BUDGET {
            return None;
        }
        return plist::from_bytes(&decoded).ok();
    }
    plist::from_bytes(bytes).ok()
}

fn redact_key(key: &str) -> bool {
    key.starts_with("mmcs-") || key.starts_with("mmcs_") || key.starts_with("decryption-key-") || matches!(key,
        "decryption-key" | "decryption_key"
        | "trabar" | "trabas" | "trabak" | "trabaoid"
        | "CloudKitDecryptionRecordKey" | "CloudKitRecordKey" | "WallpaperUpdateKey"
        | "inline-file"
    )
}

fn redacted(value: &Value) -> Json {
    match value {
        Value::String(text) => json!({"$type": "string", "length": text.len(), "redacted": true}),
        Value::Data(bytes) => opaque_data(bytes),
        Value::Boolean(_) => json!({"$type": "boolean", "redacted": true}),
        Value::Integer(_) => json!({"$type": "integer", "redacted": true}),
        Value::Real(_) => json!({"$type": "real", "redacted": true}),
        Value::Date(_) => json!({"$type": "date", "redacted": true}),
        Value::Uid(_) => json!({"$type": "uid", "redacted": true}),
        Value::Dictionary(_) => json!({"$type": "dictionary", "redacted": true}),
        Value::Array(_) => json!({"$type": "array", "redacted": true}),
        _ => json!({"$type": "unknown", "redacted": true}),
    }
}

fn opaque_data(bytes: &[u8]) -> Json {
    json!({"$type": "data", "length": bytes.len(), "redacted": true})
}

fn plist_json(value: &Value, path: &mut Vec<String>, xhtml: &mut Map<String, Json>) -> Json {
    let key = path.last().map(String::as_str).unwrap_or_default();
    let in_mmcs = path.iter().rev().nth(1).is_some_and(|parent| parent == "bpdi");
    if redact_key(key) || (in_mmcs && matches!(key, "r" | "s" | "e" | "o")) {
        return redacted(value);
    }
    match value {
        Value::Array(values) => Json::Array(values.iter().enumerate().map(|(index, item)| {
            path.push(index.to_string());
            let result = plist_json(item, path, xhtml);
            path.pop();
            result
        }).collect()),
        Value::Dictionary(dict) => Json::Object(dict.iter().map(|(name, val)| {
            path.push(name.clone());
            let result = plist_json(val, path, xhtml);
            path.pop();
            (name.clone(), result)
        }).collect()),
        Value::Boolean(value) => json!(value),
        Value::Integer(value) => value.as_unsigned().map(|n| json!(n))
            .or_else(|| value.as_signed().map(|n| json!(n))).unwrap_or(Json::Null),
        Value::Real(value) if value.is_finite() => json!(value),
        Value::Real(value) => json!({"$type": "real", "value": value.to_string()}),
        Value::Date(value) => json!({"$type": "date", "value": value.to_xml_format()}),
        Value::Uid(value) => json!({"$type": "uid", "value": value.get()}),
        Value::Data(bytes) if matches!(key, "ab" | "ati") => {
            let mut data = opaque_data(bytes);
            if let Some(value) = decode_plist(bytes).and_then(decode_archive) {
                data["decoded"] = plist_json(&value, path, xhtml);
            } else {
                data["decode"] = json!("unsupported archive");
            }
            data
        }
        Value::Data(bytes) => opaque_data(bytes),
        Value::String(text) if matches!(key, "x" | "ix" | "tx" | "epb" | "ept")
            || text.contains("<FILE") => {
            if let Some((clean, tags)) = sanitize_xml(text) {
                xhtml.insert(path.join("."), Json::Array(tags));
                Json::String(clean)
            } else {
                redacted(value)
            }
        }
        Value::String(text) => json!(text),
        _ => json!({"$type": "unknown", "redacted": true}),
    }
}

fn decode_archive(value: Value) -> Option<Value> {
    let Some(dict) = value.as_dictionary() else { return Some(value) };
    if !dict.contains_key("$archiver") {
        return Some(value);
    }
    let objects = dict.get("$objects")?.as_array()?;
    let root = dict.get("$top")?.as_dictionary()?.get("root")?;
    // A UID can point at itself, or an archive can repeatedly reference a large subtree.
    let mut budget = DECODE_BUDGET;
    expand_archive(root, objects, 64, &mut budget)
}

fn expand_archive(value: &Value, objects: &[Value], depth: usize, budget: &mut usize) -> Option<Value> {
    let depth = depth.checked_sub(1)?;
    *budget = budget.checked_sub(match value {
        Value::String(text) => text.len().saturating_add(1),
        Value::Data(bytes) => bytes.len().saturating_add(1),
        _ => 1,
    })?;
    match value {
        Value::Uid(uid) => expand_archive(objects.get(usize::try_from(uid.get()).ok()?)?, objects, depth, budget),
        Value::Array(values) => values.iter().map(|value| expand_archive(value, objects, depth, budget))
            .collect::<Option<Vec<_>>>().map(Value::Array),
        Value::Dictionary(dict) => {
            let mut resolved = plist::Dictionary::new();
            for (key, value) in dict {
                *budget = budget.checked_sub(key.len())?;
                resolved.insert(key.clone(), expand_archive(value, objects, depth, budget)?);
            }
            if let Some(keys) = resolved.get("NS.keys") {
                let keys = keys.as_array()?;
                let values = resolved.get("NS.objects")?.as_array()?;
                if keys.len() != values.len() {
                    return None;
                }
                let mut entries = plist::Dictionary::new();
                for (key, value) in keys.iter().zip(values) {
                    entries.insert(key.as_string()?.to_owned(), value.clone());
                }
                Some(Value::Dictionary(entries))
            } else {
                Some(Value::Dictionary(resolved))
            }
        }
        _ => Some(value.clone()),
    }
}

fn sanitize_xml(text: &str) -> Option<(String, Vec<Json>)> {
    // Re-emit XML tokens so there is no unredacted second copy of a FILE tag.
    let mut output = Vec::new();
    let mut writer = EmitterConfig::new().write_document_declaration(false).create_writer(&mut output);
    let mut tags = Vec::new();
    for event in EventReader::from_str(text) {
        match event.ok()? {
            XmlEvent::StartElement { name, attributes, namespace } => {
                let mut attrs = Map::new();
                let cleaned: Vec<_> = attributes.iter().map(|attribute| {
                    let key = &attribute.name.local_name;
                    if redact_key(key) {
                        attrs.insert(attribute.name.borrow().to_repr(), redacted(&Value::String(attribute.value.clone())));
                        "[REDACTED]"
                    } else {
                        attrs.insert(attribute.name.borrow().to_repr(), json!(&attribute.value));
                        attribute.value.as_str()
                    }
                }).collect();
                let mut tag = WriteEvent::start_element(name.borrow());
                for (prefix, uri) in namespace.iter() {
                    if prefix != "xml" && prefix != "xmlns" {
                        tag = tag.ns(prefix, uri);
                    }
                }
                for (attribute, value) in attributes.iter().zip(cleaned) {
                    tag = tag.attr(attribute.name.borrow(), value);
                }
                writer.write(tag).ok()?;
                tags.push(json!({"tag": name.borrow().to_repr(), "attributes": attrs}));
            }
            XmlEvent::EndElement { .. } => writer.write(WriteEvent::end_element()).ok()?,
            XmlEvent::Characters(text) | XmlEvent::Whitespace(text) | XmlEvent::CData(text) => {
                writer.write(WriteEvent::characters(&text)).ok()?;
            }
            _ => {}
        }
    }
    Some((String::from_utf8(output).ok()?, tags))
}

#[cfg(test)]
mod tests {
    use super::*;
    use plist::{Dictionary, Uid};

    fn dictionary<const N: usize>(values: [(&str, Value); N]) -> Value {
        Value::Dictionary(Dictionary::from_iter(values))
    }

    fn envelope() -> Value {
        dictionary([("c", Value::Integer(100.into()))])
    }

    #[test]
    fn captures_plist_and_file_attributes_without_mmcs_secrets() {
        let body = dictionary([
            ("t", Value::String("<3".into())),
            ("x", Value::String(r#"<html><body><FILE name="a&amp;b" width="1200" height="0" datasize="178" decryption-key="SECRET" mmcs-url="https://private.invalid/a"/></body></html>"#.into())),
            ("bpdi", dictionary([
                ("r", Value::String("https://other.private.invalid/a".into())),
                ("e", Value::Data(vec![1, 2, 3])),
                ("f", Value::Integer(178.into())),
            ])),
        ]);
        let mut bytes = Vec::new();
        plist::to_writer_binary(&mut bytes, &body).unwrap();
        let record = capture("outbound", &envelope(), Some(Body::Bytes(&bytes)), Some(0));
        assert_eq!(record["command"], 100);
        assert_eq!(record["recipientIndex"], 0);
        assert_eq!(record["plist"]["t"], "<3");
        assert_eq!(record["plist"]["bpdi"]["f"], 178);
        let file = &record["xhtml"]["x"][2]["attributes"];
        assert_eq!(file["name"], "a&b");
        assert_eq!(file["width"], "1200");
        assert_eq!(file["height"], "0");
        assert_eq!(file["datasize"], "178");
        assert_eq!(file["decryption-key"]["redacted"], true);
        assert_eq!(record["plist"]["bpdi"]["e"]["$type"], "data");
        let serialized = record.to_string();
        assert!(!serialized.contains("SECRET"));
        assert!(!serialized.contains("private.invalid"));
    }

    #[test]
    fn resolves_keyed_attributed_values_before_redacting_and_handles_invalid_xml() {
        let archive = dictionary([
            ("$archiver", "NSKeyedArchiver".into()),
            ("$top", dictionary([("root", Value::Uid(Uid::new(1)))])),
            ("$objects", Value::Array(vec![
                "$null".into(),
                dictionary([
                    ("NS.keys", Value::Array(vec![Value::Uid(Uid::new(2)), "text".into()])),
                    ("NS.objects", Value::Array(vec![Value::Uid(Uid::new(3)), "hello".into()])),
                ]),
                "mmcs-url".into(),
                "https://hidden.invalid/mmcs".into(),
            ])),
        ]);
        let mut bytes = Vec::new();
        plist::to_writer_binary(&mut bytes, &archive).unwrap();
        let body = dictionary([
            ("ab", Value::Data(bytes)),
            ("x", r#"<FILE mmcs-url="https://hidden.invalid/invalid""#.into()),
        ]);
        let record = capture("inbound", &envelope(), Some(Body::Plist(&body)), None);
        assert_eq!(record["plist"]["ab"]["decoded"]["text"], "hello");
        assert_eq!(record["plist"]["ab"]["decoded"]["mmcs-url"]["redacted"], true);
        assert_eq!(record["plist"]["x"]["redacted"], true);
        assert!(!record.to_string().contains("hidden.invalid"));

        let cyclic = dictionary([
            ("$archiver", "NSKeyedArchiver".into()),
            ("$top", dictionary([("root", Value::Uid(Uid::new(0)))])),
            ("$objects", Value::Array(vec![Value::Uid(Uid::new(0))])),
        ]);
        assert!(decode_archive(cyclic).is_none());
    }

    #[test]
    fn capture_is_opt_in_and_only_records_the_madrid_topic() {
        let directory = std::env::temp_dir().join(format!("wirecap-test-{}", uuid::Uuid::new_v4()));
        record(MADRID_TOPIC, "inbound", &envelope(), None, None);
        assert!(!is_enabled());
        set_directory(Some(directory.clone())).unwrap();
        record("com.apple.private.alloy.sms", "inbound", &envelope(), None, None);
        record(MADRID_TOPIC, "inbound", &envelope(), None, None);
        set_directory(None).unwrap();
        record(MADRID_TOPIC, "outbound", &envelope(), None, None);

        let files: Vec<_> = fs::read_dir(&directory).unwrap().map(|entry| entry.unwrap().path()).collect();
        assert_eq!(files.len(), 1);
        let content = fs::read_to_string(&files[0]).unwrap();
        fs::remove_dir_all(directory).unwrap();
        let records: Vec<Json> = content.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["direction"], "inbound");
        assert!(records[0]["plist"].is_null());
    }
}
