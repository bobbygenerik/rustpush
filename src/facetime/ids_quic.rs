//! FaceTime-only IDS logical datagram boundary.
//!
//! Runtime capture shows that this is not a standalone QUIC socket. The IDS
//! client channel is a QPod child of a QuickRelay QUIC/HTTP3 parent and may
//! later move to a P2P QPod/UDP parent without changing the logical channel.
//! The existing type names are retained for compatibility, but a concrete
//! implementation must expose ordered inner IDS datagrams only after QPod and
//! demux processing. It must not dial either observed endpoint as raw QUIC.

use std::{
    fmt,
    io::Cursor,
    net::{Ipv6Addr, SocketAddrV6},
};

use async_trait::async_trait;
use plist::Value;
use thiserror::Error;

use super::{FTClient, FTNativeSignalDirection, FTNativeSignalEnvelope, FTNativeSignalError};

const PARTICIPANT_ID_ALIAS_KEY: &str = "participantIDAlias";

/// Salt returned by Apple's `IDSIDAliasFixedSalt()` for the production
/// `unicastConnectorWithDataMode:` path.
///
/// This is connection identity material, not the TLS PSK used by the separate
/// blob-driven/test connector path.
pub const FACE_TIME_IDS_ALIAS_FIXED_SALT: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

#[derive(Debug, Error, Eq, PartialEq)]
pub enum FaceTimeIdsQuicError {
    #[error("command-207 participant data is not a property-list dictionary")]
    MalformedParticipantData,
    #[error("command-207 participant data has no participantIDAlias")]
    MissingParticipantIdAlias,
    #[error("command-207 participantIDAlias is not a non-negative integer")]
    InvalidParticipantIdAlias,
    #[error("command-207 participantIDAlias is outside the u64 range")]
    ParticipantIdAliasOutOfRange,
    #[error("FaceTime IDS QUIC session ID is empty")]
    EmptySessionId,
    #[error("FaceTime IDS QUIC session salt is empty")]
    EmptySessionSalt,
    #[error(transparent)]
    NativeSignal(#[from] FTNativeSignalError),
}

#[derive(Clone, Eq, PartialEq)]
pub struct FaceTimeParticipantMediaBundle {
    pub participant_id_alias: u64,
    pub raw_participant_data: Vec<u8>,
}

impl fmt::Debug for FaceTimeParticipantMediaBundle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FaceTimeParticipantMediaBundle")
            .field("participant_id_alias", &self.participant_id_alias)
            .field(
                "raw_participant_data_length",
                &self.raw_participant_data.len(),
            )
            .finish()
    }
}

impl FaceTimeParticipantMediaBundle {
    pub fn decode(raw_participant_data: &[u8]) -> Result<Self, FaceTimeIdsQuicError> {
        let value = Value::from_reader(Cursor::new(raw_participant_data))
            .map_err(|_| FaceTimeIdsQuicError::MalformedParticipantData)?;
        let dictionary = value
            .as_dictionary()
            .ok_or(FaceTimeIdsQuicError::MalformedParticipantData)?;
        let alias = dictionary
            .get(PARTICIPANT_ID_ALIAS_KEY)
            .ok_or(FaceTimeIdsQuicError::MissingParticipantIdAlias)?;
        let participant_id_alias = match alias {
            Value::Integer(integer) => integer
                .as_unsigned()
                .ok_or(FaceTimeIdsQuicError::InvalidParticipantIdAlias)?,
            Value::String(integer) => {
                if integer.is_empty() || !integer.bytes().all(|byte| byte.is_ascii_digit()) {
                    return Err(FaceTimeIdsQuicError::InvalidParticipantIdAlias);
                }
                integer
                    .parse::<u64>()
                    .map_err(|_| FaceTimeIdsQuicError::ParticipantIdAliasOutOfRange)?
            }
            _ => return Err(FaceTimeIdsQuicError::InvalidParticipantIdAlias),
        };

        Ok(Self {
            participant_id_alias,
            raw_participant_data: raw_participant_data.to_vec(),
        })
    }
}

pub fn face_time_ids_account_id(session_id: &str, participant_id: u64) -> String {
    format!("groupsession:{session_id}:ids:{participant_id}:L")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FaceTimeIdsQuicTlsConfiguration {
    pre_shared_key_identity: Option<&'static str>,
}

impl FaceTimeIdsQuicTlsConfiguration {
    pub const NO_PSK: Self = Self {
        pre_shared_key_identity: None,
    };

    pub fn pre_shared_key_identity(self) -> Option<&'static str> {
        self.pre_shared_key_identity
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct FaceTimeIdsQuicIdentity {
    pub session_id: String,
    pub participant_id: u64,
    pub participant_id_alias: u64,
    pub salt: Vec<u8>,
    pub account_id: String,
}

impl fmt::Debug for FaceTimeIdsQuicIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FaceTimeIdsQuicIdentity")
            .field("session_id", &self.session_id)
            .field("participant_id", &self.participant_id)
            .field("participant_id_alias", &self.participant_id_alias)
            .field("salt_length", &self.salt.len())
            .field("account_id", &self.account_id)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct FaceTimeIdsQuicConfiguration {
    pub identity: FaceTimeIdsQuicIdentity,
    pub local_address: SocketAddrV6,
    pub datagrams_enabled: bool,
    pub tls: FaceTimeIdsQuicTlsConfiguration,
}

impl fmt::Debug for FaceTimeIdsQuicConfiguration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FaceTimeIdsQuicConfiguration")
            .field("identity", &self.identity)
            .field("local_address", &self.local_address)
            .field("datagrams_enabled", &self.datagrams_enabled)
            .field("tls", &self.tls)
            .finish()
    }
}

impl FaceTimeIdsQuicConfiguration {
    pub fn assemble_for_unicast_connector(
        session_id: String,
        participant_id: u64,
        participant_id_alias: u64,
    ) -> Result<Self, FaceTimeIdsQuicError> {
        Self::assemble(
            session_id,
            participant_id,
            participant_id_alias,
            FACE_TIME_IDS_ALIAS_FIXED_SALT.to_vec(),
        )
    }

    pub fn assemble(
        session_id: String,
        participant_id: u64,
        participant_id_alias: u64,
        salt: Vec<u8>,
    ) -> Result<Self, FaceTimeIdsQuicError> {
        if session_id.is_empty() {
            return Err(FaceTimeIdsQuicError::EmptySessionId);
        }
        if salt.is_empty() {
            return Err(FaceTimeIdsQuicError::EmptySessionSalt);
        }
        let account_id = face_time_ids_account_id(&session_id, participant_id);
        Ok(Self {
            identity: FaceTimeIdsQuicIdentity {
                session_id,
                participant_id,
                participant_id_alias,
                salt,
                account_id,
            },
            local_address: SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0),
            datagrams_enabled: true,
            tls: FaceTimeIdsQuicTlsConfiguration::NO_PSK,
        })
    }

    pub fn pre_shared_key_identity(&self) -> Option<&'static str> {
        self.tls.pre_shared_key_identity()
    }
}

pub trait FaceTimeSkeDatagramReceiver: Send + Sync {
    fn receive_ske_datagram(
        &self,
        guid: String,
        ns_since_epoch: Option<u64>,
        participant_id: Option<u64>,
        direction: FTNativeSignalDirection,
        datagram: &[u8],
    ) -> Result<FTNativeSignalEnvelope, FTNativeSignalError>;
}

impl FaceTimeSkeDatagramReceiver for FTClient {
    fn receive_ske_datagram(
        &self,
        guid: String,
        ns_since_epoch: Option<u64>,
        participant_id: Option<u64>,
        direction: FTNativeSignalDirection,
        datagram: &[u8],
    ) -> Result<FTNativeSignalEnvelope, FTNativeSignalError> {
        FTClient::receive_ske_datagram(
            self,
            guid,
            ns_since_epoch,
            participant_id,
            direction,
            datagram,
        )
    }
}

#[async_trait]
/// Ordered inner-datagram boundary for the logical IDS client channel.
///
/// A production implementation must own or attach to the QuickRelay/QPod
/// transport and keep this logical connection stable while the physical path
/// switches from relay to P2P.
pub trait FaceTimeIdsQuicDatagramConnection: Send {
    async fn send_datagram(&mut self, datagram: &[u8]) -> Result<(), FaceTimeIdsQuicError>;

    async fn receive_datagram(&mut self) -> Result<Option<Vec<u8>>, FaceTimeIdsQuicError>;
}

#[derive(Clone, Debug)]
pub struct FaceTimeIdsQuicTransport {
    configuration: FaceTimeIdsQuicConfiguration,
}

impl FaceTimeIdsQuicTransport {
    pub fn new(configuration: FaceTimeIdsQuicConfiguration) -> Self {
        Self { configuration }
    }

    pub fn configuration(&self) -> &FaceTimeIdsQuicConfiguration {
        &self.configuration
    }

    pub async fn send<C: FaceTimeIdsQuicDatagramConnection>(
        &self,
        connection: &mut C,
        datagram: &[u8],
    ) -> Result<(), FaceTimeIdsQuicError> {
        connection.send_datagram(datagram).await
    }

    pub async fn receive_until_closed<C, R>(
        &self,
        connection: &mut C,
        receiver: &R,
        ns_since_epoch: Option<u64>,
    ) -> Result<usize, FaceTimeIdsQuicError>
    where
        C: FaceTimeIdsQuicDatagramConnection,
        R: FaceTimeSkeDatagramReceiver,
    {
        let mut received = 0_usize;
        while let Some(datagram) = connection.receive_datagram().await? {
            // One sequential receive loop is intentional: IDS/SKE datagram order
            // must not be changed by per-packet task spawning.
            receiver.receive_ske_datagram(
                self.configuration.identity.session_id.clone(),
                ns_since_epoch,
                Some(self.configuration.identity.participant_id),
                FTNativeSignalDirection::Inbound,
                &datagram,
            )?;
            received = received.saturating_add(1);
        }
        Ok(received)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use async_trait::async_trait;
    use plist::{Dictionary, Value};

    use super::*;
    use crate::facetime::{
        FTNativeSignal, FTNativeSignalDirection, FTNativeSignalEnvelope, FTNativeSignalError,
    };
    use crate::util::plist_to_bin;

    fn participant_blob(alias: Value) -> Vec<u8> {
        let mut dictionary = Dictionary::new();
        dictionary.insert("participantIDAlias".into(), alias);
        dictionary.insert(
            "opaqueParticipantData".into(),
            Value::Data(vec![0, 0xff, 7]),
        );
        plist_to_bin(&Value::Dictionary(dictionary)).unwrap()
    }

    fn ske_packet(sequence: u64, body: &[u8]) -> Vec<u8> {
        let mut packet = format!(
            "MESSAGE sip:peer SIP/2.0\r\nCall-ID: ids-quic-test@addressid\r\nCSeq: 7 MESSAGE\r\nSKESeq: {sequence};0\r\nContent-Type: application/ske\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        packet.extend_from_slice(body);
        packet
    }

    #[test]
    fn command_207_bundle_decodes_u64_alias_and_preserves_raw_data() {
        let raw = participant_blob(Value::Integer(u64::MAX.into()));
        let bundle = FaceTimeParticipantMediaBundle::decode(&raw).unwrap();

        assert_eq!(bundle.participant_id_alias, u64::MAX);
        assert_eq!(bundle.raw_participant_data, raw);
    }

    #[test]
    fn command_207_bundle_rejects_missing_invalid_and_out_of_range_aliases() {
        let missing = plist_to_bin(&Value::Dictionary(Dictionary::new())).unwrap();
        assert!(matches!(
            FaceTimeParticipantMediaBundle::decode(&missing),
            Err(FaceTimeIdsQuicError::MissingParticipantIdAlias)
        ));

        let invalid = participant_blob(Value::Integer((-1_i64).into()));
        assert!(matches!(
            FaceTimeParticipantMediaBundle::decode(&invalid),
            Err(FaceTimeIdsQuicError::InvalidParticipantIdAlias)
        ));

        let out_of_range = participant_blob(Value::String("18446744073709551616".into()));
        assert!(matches!(
            FaceTimeParticipantMediaBundle::decode(&out_of_range),
            Err(FaceTimeIdsQuicError::ParticipantIdAliasOutOfRange)
        ));

        let malformed = b"not a property list";
        let error = FaceTimeParticipantMediaBundle::decode(malformed).unwrap_err();
        assert!(matches!(
            error,
            FaceTimeIdsQuicError::MalformedParticipantData
        ));
        assert!(!format!("{error:?}").contains("not a property list"));
    }

    #[test]
    fn account_id_matches_the_pinned_apple_format_exactly() {
        assert_eq!(
            face_time_ids_account_id("session-uuid", u64::MAX),
            "groupsession:session-uuid:ids:18446744073709551615:L"
        );
    }

    #[test]
    fn transport_configuration_assembles_known_identity_without_psk() {
        let configuration =
            FaceTimeIdsQuicConfiguration::assemble("session-uuid".into(), 41, 73, vec![1, 2, 3, 4])
                .unwrap();

        assert_eq!(configuration.identity.session_id, "session-uuid");
        assert_eq!(configuration.identity.participant_id, 41);
        assert_eq!(configuration.identity.participant_id_alias, 73);
        assert_eq!(configuration.identity.salt, [1, 2, 3, 4]);
        assert_eq!(
            configuration.identity.account_id,
            "groupsession:session-uuid:ids:41:L"
        );
        assert_eq!(
            configuration.local_address.ip(),
            &std::net::Ipv6Addr::UNSPECIFIED
        );
        assert_eq!(configuration.local_address.port(), 0);
        assert!(configuration.datagrams_enabled);
        assert_eq!(configuration.tls, FaceTimeIdsQuicTlsConfiguration::NO_PSK);
        assert_eq!(configuration.pre_shared_key_identity(), None);
    }

    #[test]
    fn production_unicast_connector_uses_apple_fixed_alias_salt() {
        let configuration = FaceTimeIdsQuicConfiguration::assemble_for_unicast_connector(
            "session-uuid".into(),
            41,
            73,
        )
        .unwrap();

        assert_eq!(configuration.identity.salt, FACE_TIME_IDS_ALIAS_FIXED_SALT);
        assert_eq!(configuration.pre_shared_key_identity(), None);
    }

    struct ReplayedConnection {
        datagrams: VecDeque<Vec<u8>>,
    }

    #[async_trait]
    impl FaceTimeIdsQuicDatagramConnection for ReplayedConnection {
        async fn send_datagram(&mut self, _datagram: &[u8]) -> Result<(), FaceTimeIdsQuicError> {
            Ok(())
        }

        async fn receive_datagram(&mut self) -> Result<Option<Vec<u8>>, FaceTimeIdsQuicError> {
            Ok(self.datagrams.pop_front())
        }
    }

    #[derive(Default)]
    struct RecordingSkeReceiver {
        sequences: std::sync::Mutex<Vec<u64>>,
    }

    impl FaceTimeSkeDatagramReceiver for RecordingSkeReceiver {
        fn receive_ske_datagram(
            &self,
            guid: String,
            ns_since_epoch: Option<u64>,
            participant_id: Option<u64>,
            direction: FTNativeSignalDirection,
            datagram: &[u8],
        ) -> Result<FTNativeSignalEnvelope, FTNativeSignalError> {
            let signal = crate::facetime::native_signaling::parse_ske_sip_message(
                datagram,
                participant_id,
                direction,
            )?;
            let FTNativeSignal::SkeMessage { sequence, .. } = signal else {
                unreachable!()
            };
            self.sequences.lock().unwrap().push(sequence);
            Ok(FTNativeSignalEnvelope {
                guid,
                order: sequence,
                ns_since_epoch,
                signal,
            })
        }
    }

    #[tokio::test]
    async fn replayed_inbound_datagrams_reach_ske_receiver_in_receive_order() {
        let configuration =
            FaceTimeIdsQuicConfiguration::assemble("call-guid".into(), 41, 73, vec![1, 2, 3, 4])
                .unwrap();
        let transport = FaceTimeIdsQuicTransport::new(configuration);
        let receiver = RecordingSkeReceiver::default();
        let mut connection = ReplayedConnection {
            datagrams: VecDeque::from([ske_packet(10, &[0, 0xff]), ske_packet(11, &[1, 0x80])]),
        };

        let received = transport
            .receive_until_closed(&mut connection, &receiver, Some(900))
            .await
            .unwrap();

        assert_eq!(received, 2);
        assert_eq!(*receiver.sequences.lock().unwrap(), [10, 11]);
    }
}
