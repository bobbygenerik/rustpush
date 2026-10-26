use rtc_rtcp::{header::{Header, PacketType, HEADER_LENGTH}, packet::Packet, raw_packet::RawPacket};
use rtc_shared::{error::{Error, Result}, marshal::Unmarshal};

/// Receive-side diagnostics only. The timestamp is an RTP reference, not a
/// decoder receipt or permission to acknowledge an encoder reference slot.
/// iOS27 __VideoReceiver_ProcessVideoRTCPInternal / ProcessRtcpLTRAck.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ApplicationFeedback {
    LtrAck { timestamp: u32 },
    Loss { timestamp: u32, received_packets: u16, frame_size: u8, lost_packets: u8 },
    Unknown { identifier: Option<u32> },
}

pub(super) fn application_feedback(packet: &[u8]) -> Result<ApplicationFeedback> {
    let header = Header::unmarshal(&mut &packet[..])?;
    let length = (usize::from(header.length) + 1) * HEADER_LENGTH;
    if header.packet_type != PacketType::PayloadSpecificFeedback || header.count != 15 {
        return Err(Error::InvalidHeader);
    }
    let packet = packet.get(..length).ok_or(Error::PacketTooShort)?;
    // RTCP padding is not application data. It cannot satisfy the known FCI
    // layouts or borrow bytes from the next packet in a compound datagram.
    let padding = if header.padding {
        let count = usize::from(*packet.last().ok_or(Error::PacketTooShort)?);
        if count == 0 || count > packet.len().saturating_sub(12) {
            return Err(Error::PacketTooShort);
        }
        count
    } else { 0 };
    let body = packet.get(..packet.len() - padding).filter(|body| body.len() >= 12)
        .ok_or(Error::PacketTooShort)?;
    let identifier = body.get(12..16).map(|bytes| u32::from_be_bytes(bytes.try_into().unwrap()));
    match identifier {
        Some(kind @ (5 | 6)) => {
            let payload = body.get(16..if kind == 5 { 20 } else { 24 }).ok_or(Error::PacketTooShort)?;
            let timestamp = u32::from_be_bytes(payload[..4].try_into().unwrap());
            Ok(if kind == 5 { ApplicationFeedback::LtrAck { timestamp } } else {
                ApplicationFeedback::Loss { timestamp,
                    received_packets: u16::from_be_bytes(payload[4..6].try_into().unwrap()),
                    frame_size: payload[6], lost_packets: payload[7] }
            })
        }
        identifier => Ok(ApplicationFeedback::Unknown { identifier }),
    }
}

/// FaceTime accepts application feedback other than REMB. iOS27's
/// __RTCPAddPSFBAlfbPacket writes numeric FCI identifiers5/6 with PT206/FMT15;
/// that shared format alone does not identify a REMB payload.
/// Keep the vendored WebRTC parser and recognized-packet validation unchanged.
pub(super) fn parse(mut data: &[u8]) -> Result<Vec<Box<dyn Packet>>> {
    if data.is_empty() { return Err(Error::InvalidHeader); }
    let mut packets: Vec<Box<dyn Packet>> = Vec::new();
    while !data.is_empty() {
        let header = Header::unmarshal(&mut &data[..])?;
        let length = (usize::from(header.length) + 1) * HEADER_LENGTH;
        let packet = data.get(..length).ok_or(Error::PacketTooShort)?;
        let application_feedback = header.packet_type == PacketType::PayloadSpecificFeedback
            && header.count == 15;
        if application_feedback && packet.len() < 12 { return Err(Error::PacketTooShort); }
        if application_feedback && packet.get(12..16) != Some(b"REMB".as_slice()) {
            // Opaque means unsupported, not malformed. Do not zero its media
            // SSRC, interpret it as bitrate feedback, or discard adjacent NACKs.
            self::application_feedback(packet)?;
            packets.push(Box::new(RawPacket::unmarshal(&mut &packet[..])?));
        } else {
            packets.extend(rtc_rtcp::packet::unmarshal(&mut &packet[..])?);
        }
        data = &data[length..];
    }
    Ok(packets)
}
