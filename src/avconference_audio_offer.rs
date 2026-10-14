//! U1 outbound codec admission from the remote microphone capability bitmap.
//! iOS 27 AVConference flagFromPayload: (0x2063dc6e8) maps AAC/PT104 to bit 0.
//! This guards our existing AAC encoder; it is not Apple's full codec chooser.
use std::sync::atomic::{AtomicU64, Ordering};

const KNOWN_OFFER: u64 = 1 << 32;
const AAC_PAYLOAD_FLAG: u32 = 1;
// iOS 27 payloads getter at 0x2063c318c returns 0xf when the field is absent.
const DEFAULT_U1_PAYLOADS: u32 = 15;

#[derive(Default)]
pub(super) struct RemoteAudioOffer {
    // Explicit zero is a known empty offer, distinct from no U1 settings yet.
    bitmap: AtomicU64,
}

impl RemoteAudioOffer {
    pub(super) fn configure(&self, microphone: Option<Option<u32>>) {
        if let Some(bitmap) = microphone {
            let bitmap = bitmap.unwrap_or(DEFAULT_U1_PAYLOADS);
            self.bitmap.store(KNOWN_OFFER | u64::from(bitmap), Ordering::Release);
        }
        // Missing whole U1 settings must not erase a prior explicit offer.
        // Legacy-only negotiation remains on the existing compatibility path.
    }

    pub(super) fn ensure_aac_supported(&self) -> std::io::Result<()> {
        let offer = self.bitmap.load(Ordering::Acquire);
        if offer & KNOWN_OFFER != 0 && offer as u32 & AAC_PAYLOAD_FLAG == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::Unsupported,
                format!("FaceTime peer microphone offer {:#x} does not support outbound AAC payload 104", offer as u32)));
        }
        Ok(())
    }
}
