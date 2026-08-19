use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, LazyLock, Mutex},
};

use serde::Serialize;
use thiserror::Error;
use tokio::sync::broadcast;

const NATIVE_SIGNAL_CAPACITY: usize = 128;
const MAXIMUM_SKE_BODY_LENGTH: usize = 900;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FTNativeSignalDirection {
    Inbound,
    Outbound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FTSipSkeMessageType {
    Request,
    Status,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[repr(u32)]
pub enum FTQuickRelayMaterialType {
    AvcBlobPlain = 10,
    Prekey = 11,
    AvcBlob = 12,
    Mkm = 13,
    Skm = 14,
}

impl TryFrom<u32> for FTQuickRelayMaterialType {
    type Error = FTNativeSignalError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            10 => Ok(Self::AvcBlobPlain),
            11 => Ok(Self::Prekey),
            12 => Ok(Self::AvcBlob),
            13 => Ok(Self::Mkm),
            14 => Ok(Self::Skm),
            _ => Err(FTNativeSignalError::UnsupportedMaterialType(value)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FTQuickRelayAllocationToken {
    pub participant_id: u64,
    pub participant_handle: String,
    pub token: Vec<u8>,
}

#[derive(Clone, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum FTNativeSignal {
    ParticipantBundle {
        participant_id: u64,
        sender: String,
        participant_data: Vec<u8>,
    },
    ParticipantChanged {
        participant_id: u64,
        handle: String,
        joined: bool,
    },
    QuickRelayAllocation {
        relay_ip: Vec<u8>,
        relay_port: u16,
        relay_session_id: Vec<u8>,
        relay_session_key: Vec<u8>,
        relay_session_token: Vec<u8>,
        relay_credential: Option<Vec<u8>>,
        allocations: Vec<FTQuickRelayAllocationToken>,
    },
    SkeMessage {
        participant_id: Option<u64>,
        direction: FTNativeSignalDirection,
        sip_call_id: String,
        cseq: u32,
        sequence: u64,
        fragment_index: u32,
        message_type: FTSipSkeMessageType,
        status: Option<u16>,
        body: Vec<u8>,
    },
    QuickRelayMaterial {
        relay_group_id: String,
        owner_participant_id: Option<u64>,
        receiver_participant_id: Option<u64>,
        material_type: FTQuickRelayMaterialType,
        material: Vec<u8>,
    },
    CallTerminated {
        reason: String,
    },
}

impl fmt::Debug for FTNativeSignal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParticipantBundle {
                participant_id,
                sender,
                participant_data,
            } => formatter
                .debug_struct("ParticipantBundle")
                .field("participant_id", participant_id)
                .field("sender", sender)
                .field("participant_data_length", &participant_data.len())
                .finish(),
            Self::ParticipantChanged {
                participant_id,
                handle,
                joined,
            } => formatter
                .debug_struct("ParticipantChanged")
                .field("participant_id", participant_id)
                .field("handle", handle)
                .field("joined", joined)
                .finish(),
            Self::QuickRelayAllocation {
                relay_ip,
                relay_port,
                relay_session_id,
                relay_session_key,
                relay_session_token,
                relay_credential,
                allocations,
            } => formatter
                .debug_struct("QuickRelayAllocation")
                .field("relay_ip", relay_ip)
                .field("relay_port", relay_port)
                .field("relay_session_id_length", &relay_session_id.len())
                .field("relay_session_key_length", &relay_session_key.len())
                .field("relay_session_token_length", &relay_session_token.len())
                .field(
                    "relay_credential_length",
                    &relay_credential.as_ref().map(Vec::len),
                )
                .field("allocation_count", &allocations.len())
                .finish(),
            Self::SkeMessage {
                participant_id,
                direction,
                sip_call_id,
                cseq,
                sequence,
                fragment_index,
                message_type,
                status,
                body,
            } => formatter
                .debug_struct("SkeMessage")
                .field("participant_id", participant_id)
                .field("direction", direction)
                .field("sip_call_id", sip_call_id)
                .field("cseq", cseq)
                .field("sequence", sequence)
                .field("fragment_index", fragment_index)
                .field("message_type", message_type)
                .field("status", status)
                .field("body_length", &body.len())
                .finish(),
            Self::QuickRelayMaterial {
                relay_group_id,
                owner_participant_id,
                receiver_participant_id,
                material_type,
                material,
            } => formatter
                .debug_struct("QuickRelayMaterial")
                .field("relay_group_id", relay_group_id)
                .field("owner_participant_id", owner_participant_id)
                .field("receiver_participant_id", receiver_participant_id)
                .field("material_type", material_type)
                .field("material_length", &material.len())
                .finish(),
            Self::CallTerminated { reason } => formatter
                .debug_struct("CallTerminated")
                .field("reason", reason)
                .finish(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FTNativeSignalEnvelope {
    pub guid: String,
    pub order: u64,
    pub ns_since_epoch: Option<u64>,
    pub signal: FTNativeSignal,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum FTNativeSignalError {
    #[error("native FaceTime call GUID is empty")]
    EmptyGuid,
    #[error("malformed SIP MESSAGE: {0}")]
    MalformedSip(&'static str),
    #[error("unsupported QuickRelay material type {0}")]
    UnsupportedMaterialType(u32),
    #[error("QuickRelay material is empty")]
    EmptyMaterial,
}

#[derive(Clone)]
pub struct FTNativeSignalChannel {
    sender: broadcast::Sender<FTNativeSignalEnvelope>,
    call_orders: Arc<Mutex<HashMap<String, u64>>>,
}

impl Default for FTNativeSignalChannel {
    fn default() -> Self {
        let (sender, _) = broadcast::channel(NATIVE_SIGNAL_CAPACITY);
        Self {
            sender,
            call_orders: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl FTNativeSignalChannel {
    pub fn subscribe(&self) -> broadcast::Receiver<FTNativeSignalEnvelope> {
        self.sender.subscribe()
    }

    pub fn publish(
        &self,
        guid: String,
        ns_since_epoch: Option<u64>,
        signal: FTNativeSignal,
    ) -> Result<FTNativeSignalEnvelope, FTNativeSignalError> {
        if guid.is_empty() {
            return Err(FTNativeSignalError::EmptyGuid);
        }
        let order = {
            let mut orders = self.call_orders.lock().expect("native signal order lock");
            let next = orders.entry(guid.clone()).or_default();
            let order = *next;
            *next = next
                .checked_add(1)
                .expect("native FaceTime order exhausted");
            order
        };
        let envelope = FTNativeSignalEnvelope {
            guid,
            order,
            ns_since_epoch,
            signal,
        };
        // Tokio broadcast intentionally treats an absent consumer as a drop,
        // never as backpressure on the lifecycle receive path.
        let _ = self.sender.send(envelope.clone());
        Ok(envelope)
    }
}

static NATIVE_SIGNALS: LazyLock<FTNativeSignalChannel> =
    LazyLock::new(FTNativeSignalChannel::default);

pub fn subscribe_native_signals() -> broadcast::Receiver<FTNativeSignalEnvelope> {
    NATIVE_SIGNALS.subscribe()
}

pub fn publish_native_signal(
    guid: String,
    ns_since_epoch: Option<u64>,
    signal: FTNativeSignal,
) -> Result<FTNativeSignalEnvelope, FTNativeSignalError> {
    NATIVE_SIGNALS.publish(guid, ns_since_epoch, signal)
}

pub fn parse_ske_sip_message(
    bytes: &[u8],
    participant_id: Option<u64>,
    direction: FTNativeSignalDirection,
) -> Result<FTNativeSignal, FTNativeSignalError> {
    let separator = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or(FTNativeSignalError::MalformedSip(
            "missing header terminator",
        ))?;
    let header_bytes = &bytes[..separator + 2];
    let body = bytes[separator + 4..].to_vec();
    if body.len() > MAXIMUM_SKE_BODY_LENGTH {
        return Err(FTNativeSignalError::MalformedSip(
            "SKE body exceeds 900 bytes",
        ));
    }
    let headers = std::str::from_utf8(header_bytes)
        .map_err(|_| FTNativeSignalError::MalformedSip("headers are not UTF-8/ASCII"))?;
    let mut lines = headers.split("\r\n");
    let first_line = lines
        .next()
        .ok_or(FTNativeSignalError::MalformedSip("missing start line"))?;
    let (message_type, status) = if first_line.starts_with("MESSAGE ") {
        (FTSipSkeMessageType::Request, None)
    } else if let Some(rest) = first_line.strip_prefix("SIP/") {
        let status = rest
            .split_whitespace()
            .nth(1)
            .ok_or(FTNativeSignalError::MalformedSip("missing SIP status"))?
            .parse::<u16>()
            .map_err(|_| FTNativeSignalError::MalformedSip("invalid SIP status"))?;
        (FTSipSkeMessageType::Status, Some(status))
    } else {
        return Err(FTNativeSignalError::MalformedSip(
            "not a SIP MESSAGE/status",
        ));
    };

    let mut call_id = None;
    let mut cseq = None;
    let mut ske_seq = None;
    let mut content_type = None;
    let mut content_length = None;
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or(FTNativeSignalError::MalformedSip("malformed header"))?;
        let value = value.trim();
        match name.to_ascii_lowercase().as_str() {
            "call-id" => call_id = Some(value.to_string()),
            "content-type" => content_type = Some(value),
            "content-length" => {
                content_length = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| FTNativeSignalError::MalformedSip("invalid Content-Length"))?,
                )
            }
            "cseq" => {
                let mut parts = value.split_whitespace();
                let number = parts
                    .next()
                    .ok_or(FTNativeSignalError::MalformedSip("missing CSeq"))?
                    .parse::<u32>()
                    .map_err(|_| FTNativeSignalError::MalformedSip("invalid CSeq"))?;
                if !matches!(parts.next(), Some(method) if method.eq_ignore_ascii_case("MESSAGE")) {
                    return Err(FTNativeSignalError::MalformedSip(
                        "CSeq method is not MESSAGE",
                    ));
                }
                cseq = Some(number);
            }
            "skeseq" => {
                let (sequence, fragment) = value
                    .split_once(';')
                    .ok_or(FTNativeSignalError::MalformedSip("malformed SKESeq"))?;
                ske_seq = Some((
                    sequence
                        .parse::<u64>()
                        .map_err(|_| FTNativeSignalError::MalformedSip("invalid SKE sequence"))?,
                    fragment.parse::<u32>().map_err(|_| {
                        FTNativeSignalError::MalformedSip("invalid SKE fragment index")
                    })?,
                ));
            }
            _ => {}
        }
    }
    if content_type != Some("application/ske") {
        return Err(FTNativeSignalError::MalformedSip(
            "Content-Type is not application/ske",
        ));
    }
    if content_length.is_some_and(|length| length != body.len()) {
        return Err(FTNativeSignalError::MalformedSip("Content-Length mismatch"));
    }
    let (sequence, fragment_index) =
        ske_seq.ok_or(FTNativeSignalError::MalformedSip("missing SKESeq"))?;

    Ok(FTNativeSignal::SkeMessage {
        participant_id,
        direction,
        sip_call_id: call_id.ok_or(FTNativeSignalError::MalformedSip("missing Call-ID"))?,
        cseq: cseq.ok_or(FTNativeSignalError::MalformedSip("missing CSeq"))?,
        sequence,
        fragment_index,
        message_type,
        status,
        body,
    })
}

pub fn quickrelay_material_signal(
    relay_group_id: String,
    owner_participant_id: Option<u64>,
    receiver_participant_id: Option<u64>,
    material_type: u32,
    material: Vec<u8>,
) -> Result<FTNativeSignal, FTNativeSignalError> {
    if relay_group_id.is_empty() {
        return Err(FTNativeSignalError::EmptyGuid);
    }
    if material.is_empty() {
        return Err(FTNativeSignalError::EmptyMaterial);
    }
    Ok(FTNativeSignal::QuickRelayMaterial {
        relay_group_id,
        owner_participant_id,
        receiver_participant_id,
        material_type: material_type.try_into()?,
        material,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ske_packet(body: &[u8]) -> Vec<u8> {
        let mut packet = format!(
            "MESSAGE sip:peer SIP/2.0\r\nCall-ID: 550e8400-e29b-41d4-a716-446655440000@addressid\r\nCSeq: 7 MESSAGE\r\nSKESeq: 18446744073709551615;4294967295\r\nContent-Type: application/ske\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        packet.extend_from_slice(body);
        packet
    }

    #[test]
    fn ske_parser_preserves_binary_body_and_full_sequence_ranges() {
        let signal = parse_ske_sip_message(
            &ske_packet(&[0, 0xff, 0x80]),
            Some(u64::MAX),
            FTNativeSignalDirection::Inbound,
        )
        .unwrap();
        let FTNativeSignal::SkeMessage {
            participant_id,
            sequence,
            fragment_index,
            body,
            ..
        } = signal
        else {
            panic!("expected SKE message")
        };
        assert_eq!(participant_id, Some(u64::MAX));
        assert_eq!(sequence, u64::MAX);
        assert_eq!(fragment_index, u32::MAX);
        assert_eq!(body, [0, 0xff, 0x80]);
    }

    #[test]
    fn malformed_ske_fails_closed_without_payload_logging() {
        let error = parse_ske_sip_message(
            b"MESSAGE sip:peer SIP/2.0\r\nContent-Type: text/plain\r\n\r\nsecret",
            None,
            FTNativeSignalDirection::Inbound,
        )
        .unwrap_err();
        assert!(matches!(error, FTNativeSignalError::MalformedSip(_)));
        assert!(!format!("{error:?}").contains("secret"));
    }

    #[test]
    fn material_types_ten_through_fourteen_are_preserved() {
        for value in 10..=14 {
            let signal = quickrelay_material_signal(
                "call".to_string(),
                Some(1),
                Some(2),
                value,
                vec![0, value as u8, 0xff],
            )
            .unwrap();
            let FTNativeSignal::QuickRelayMaterial {
                material_type,
                material,
                ..
            } = signal
            else {
                panic!("expected material")
            };
            assert_eq!(material_type as u32, value);
            assert_eq!(material, [0, value as u8, 0xff]);
        }
        assert!(quickrelay_material_signal("call".into(), None, None, 9, vec![1]).is_err());
    }

    #[tokio::test]
    async fn channel_is_additive_nonblocking_and_reports_slow_consumers() {
        let channel = FTNativeSignalChannel::default();
        channel
            .publish(
                "no-consumer".into(),
                Some(1),
                FTNativeSignal::ParticipantBundle {
                    participant_id: 1,
                    sender: "tel:+1".into(),
                    participant_data: vec![1],
                },
            )
            .unwrap();

        let mut slow = channel.subscribe();
        for timestamp in 0..(NATIVE_SIGNAL_CAPACITY as u64 + 1) {
            channel
                .publish(
                    "slow".into(),
                    Some(timestamp),
                    FTNativeSignal::ParticipantBundle {
                        participant_id: timestamp,
                        sender: "tel:+1".into(),
                        participant_data: vec![1],
                    },
                )
                .unwrap();
        }
        assert!(matches!(
            slow.recv().await,
            Err(broadcast::error::RecvError::Lagged(1))
        ));
    }

    #[test]
    fn json_wire_preserves_binary_bytes_without_text_or_base64_conversion() {
        let envelope = FTNativeSignalEnvelope {
            guid: "call-guid".to_string(),
            order: u64::MAX,
            ns_since_epoch: Some(u64::MAX),
            signal: FTNativeSignal::SkeMessage {
                participant_id: Some(u64::MAX),
                direction: FTNativeSignalDirection::Inbound,
                sip_call_id: "01234567-89ab-cdef-0123-456789abcdef@addressid".to_string(),
                cseq: 7,
                sequence: u64::MAX,
                fragment_index: u32::MAX,
                message_type: FTSipSkeMessageType::Request,
                status: None,
                body: vec![0, 255, 128, 1],
            },
        };

        let encoded = serde_json::to_string(&envelope).unwrap();
        assert!(encoded.contains("\"body\":[0,255,128,1]"));
        assert!(encoded.contains("\"sequence\":18446744073709551615"));
        assert!(!encoded.contains("AP+A"));
        assert!(!encoded.contains('�'));
    }
}
