//! FaceTime TX Long-Term Reference (LTR) state machine matching iOS 27.0.1 (`24A446`, `iPhone17,5`).
//!
//! Sources:
//! - `AVConference`: `__videoTransmitter_HandleLTRFrame` (`0x20601dc38`),
//!   `_VideoTransmitter_ACKLTRFrame` (`0x206025638`),
//!   `__VideoTransmitter_UpdateEncoderArgsAndGetStream` (`0x206022f60`),
//!   `-[VCVideoTransmitterDefault generateKeyFrameWithFIRType:]` (`0x2061c1ce0`),
//!   `_VideoReceiver_ProcessRtcpLTRAck` (`0x206008aa0`).
//! - `VideoProcessing`: RTT EWMA & timeout (`0x22a6c6094..0x22a6c64e0`),
//!   default window init (`0x22a6c3f0c`), `ForceRefresh` selection (`0x22a6f2e10..34`).

use std::collections::VecDeque;
use std::time::Instant;

/// `__videoTransmitter_HandleLTRFrame` (`0x20601dc38`) / `_VideoTransmitter_ACKLTRFrame` (`0x206025638`).
pub const TX_LTR_RING_CAPACITY: usize = 100;

/// Default RTT / ACK timeout window (`0x22a6c3f0c`: `mov w8, #0x7d0`).
pub const DEFAULT_LTR_RTT_TIMEOUT_MS: u32 = 2000;

/// Minimum and maximum clamped RTT window when EWMA is active (`0x22a6c6250..68`).
const MIN_LTR_RTT_MS: f64 = 1.0;
const MAX_LTR_RTT_MS: f64 = 60_000.0;

/// `0x22a6c61d0..e4`: `rtt_ewma = 0.875 * prev + 0.125 * sample`.
const RTT_EWMA_PREV_WEIGHT: f64 = 0.875;
const RTT_EWMA_SAMPLE_WEIGHT: f64 = 0.125;

/// `0x22a6c62f4`: `lsl w20, w8, #1` -> timed out after `> 2 * rtt_or_timeout_ms`.
pub const LTR_TIMEOUT_MULTIPLIER: f64 = 2.0;

/// `FTVideoControlData.ltr_bits` flags (`__VideoTransmitter_TransmitEncodedVideoFrame`):
/// - bit 0 (`0x20601a980`): `RequireLTRAcknowledgementToken`
/// - bit 1 (`0x20601a8a8`): `ReferenceWasRefreshed`
pub const LTR_BIT_MARK: u8 = 0x01;
pub const LTR_BIT_REFRESH: u8 = 0x02;

/// `VCGenerateKeyFrame.fir_type` (`0x2061c1ce0`):
/// `FIRType == 2` requests LTR refresh (`ForceRefresh`); other values request hard IDR (`ForceKeyFrame`).
pub const FIR_TYPE_IDR: u32 = 1;
pub const FIR_TYPE_LTR_REFRESH: u32 = 2;
pub const FIR_TYPE_FORCE_IDR_UNTHROTTLED: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
struct LtrSentEntry {
    token: u32,
    rtp_ts: u32,
    sent_at: Instant,
    capture_order: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LtrAckedEntry {
    token: u32,
    acked_at: Instant,
    capture_order: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LtrAckEvent {
    pub token: u32,
    pub rtp_ts: u32,
    pub sample_ms: f64,
    pub rtt_ms: u32,
}

/// Sender-side 100-entry `(token, rtp_ts)` correlation ring, RTT EWMA estimator,
/// and confirmed-ACK token tracker for `FIRType == 2` LTR refresh.
#[derive(Debug, Clone)]
pub struct LtrTxTracker {
    pending: VecDeque<LtrSentEntry>,
    acked: VecDeque<LtrAckedEntry>,
    rtt_ewma_ms: Option<f64>,
    rtt_or_timeout_ms: u32,
    max_acked_order: Option<u64>,
    next_order: u64,
}

impl Default for LtrTxTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl LtrTxTracker {
    pub fn new() -> Self {
        Self {
            pending: VecDeque::with_capacity(TX_LTR_RING_CAPACITY),
            acked: VecDeque::with_capacity(TX_LTR_RING_CAPACITY),
            rtt_ewma_ms: None,
            rtt_or_timeout_ms: DEFAULT_LTR_RTT_TIMEOUT_MS,
            max_acked_order: None,
            next_order: 1,
        }
    }

    pub fn rtt_or_timeout_ms(&self) -> u32 {
        self.rtt_or_timeout_ms
    }

    pub fn rtt_ewma_ms(&self) -> Option<f64> {
        self.rtt_ewma_ms
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn acked_len(&self) -> usize {
        self.acked.len()
    }

    /// Clear in-flight and ACKed TX LTR rings (`__VideoTransmitter_UpdateEncoderArgsAndGetStream` `0x206022f60..80`).
    /// Keeps the session-level RTT EWMA intact.
    pub fn reset_rings(&mut self) {
        self.pending.clear();
        self.acked.clear();
        self.max_acked_order = None;
    }

    /// Record an outgoing encoded video frame at RTP `rtp_ts`.
    /// On IDR (`is_key_frame`), resets rings first per `0x206022f60`, then records if `(ltr_bits & 1) != 0`.
    pub fn record_sent(
        &mut self,
        is_key_frame: bool,
        ltr_bits: u8,
        token: Option<u32>,
        rtp_ts: u32,
        now: Instant,
    ) {
        if is_key_frame {
            self.reset_rings();
        }
        if (ltr_bits & LTR_BIT_MARK) != 0 {
            if let Some(token) = token {
                if self.pending.len() >= TX_LTR_RING_CAPACITY {
                    self.pending.pop_front();
                }
                let order = self.next_order;
                self.next_order = self.next_order.wrapping_add(1);
                self.pending.push_back(LtrSentEntry {
                    token,
                    rtp_ts,
                    sent_at: now,
                    capture_order: order,
                });
            }
        }
    }

    /// Consume inbound RTCP PSFB `FMT=15` Type-5 `LtrAck { timestamp }` (`_VideoTransmitter_ACKLTRFrame` `0x206025638`).
    pub fn ack_rtp_timestamp(&mut self, rtp_ts: u32, now: Instant) -> Option<LtrAckEvent> {
        let idx = self.pending.iter().position(|e| e.rtp_ts == rtp_ts)?;
        let entry = self.pending.remove(idx)?;
        let sample_ms = now
            .saturating_duration_since(entry.sent_at)
            .as_secs_f64()
            * 1000.0;
        let next_ewma = match self.rtt_ewma_ms {
            Some(prev) => prev * RTT_EWMA_PREV_WEIGHT + sample_ms * RTT_EWMA_SAMPLE_WEIGHT,
            None => sample_ms,
        };
        self.rtt_ewma_ms = Some(next_ewma);
        self.rtt_or_timeout_ms = if next_ewma > 0.0 {
            next_ewma
                .round()
                .clamp(MIN_LTR_RTT_MS, MAX_LTR_RTT_MS) as u32
        } else {
            DEFAULT_LTR_RTT_TIMEOUT_MS
        };
        self.max_acked_order = Some(
            self.max_acked_order
                .map_or(entry.capture_order, |prev| prev.max(entry.capture_order)),
        );
        if self.acked.len() >= TX_LTR_RING_CAPACITY {
            self.acked.pop_front();
        }
        self.acked.push_back(LtrAckedEntry {
            token: entry.token,
            acked_at: now,
            capture_order: entry.capture_order,
        });
        self.expire_stale_pending(now);
        Some(LtrAckEvent {
            token: entry.token,
            rtp_ts,
            sample_ms,
            rtt_ms: self.rtt_or_timeout_ms,
        })
    }

    /// Return the newest confirmed ACKed LTR token for `FIRType == 2` refresh (`0x22a6f2e10..34`),
    /// or `None` when no ACKed LTR slot exists (caller must fall back to IDR).
    pub fn select_refresh_token(&mut self, now: Instant) -> Option<u32> {
        self.expire_stale_pending(now);
        self.acked.back().map(|e| e.token)
    }

    /// Promote stale `PendingACK` entries out of the pending list when older than `max_acked_order`
    /// (`0x22a6c648c`) or elapsed `> 2 * rtt_or_timeout_ms` (`0x22a6c62f4`).
    pub fn expire_stale_pending(&mut self, now: Instant) {
        let timeout_ms = f64::from(self.rtt_or_timeout_ms) * LTR_TIMEOUT_MULTIPLIER;
        let max_order = self.max_acked_order;
        self.pending.retain(|entry| {
            let elapsed_ms = now
                .saturating_duration_since(entry.sent_at)
                .as_secs_f64()
                * 1000.0;
            let older_than_acked = max_order.is_some_and(|max_o| entry.capture_order < max_o);
            !older_than_acked && elapsed_ms <= timeout_ms
        });
    }
}
