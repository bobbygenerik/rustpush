use std::collections::HashMap;

pub const MAX_TRACKED_FRAMES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameControlEntry<C> {
    pub ltr_bits: u8,
    pub ltr_timestamp: Option<u32>,
    pub camera_meta: Option<C>,
}

struct TrackedEntry<C> {
    control: FrameControlEntry<C>,
    order: u64,
}

/// Per-`(ssrc, rtp_timestamp)` control-info tracker matching Apple's
/// `_VideoPacketBuffer_AddPacket` (`0x205fd55e8`) attribution rules:
/// - `ltr_bits` (`frame + 0x20`): set once by the first arriving packet of the frame (`0x205fd6404..408`);
///   subsequent packets for the same frame do not overwrite or OR into it (`loc_205fd5f38`).
/// - `camera_meta` (`frame + 0x24`): first non-zero `cameraStatusBits` wins (`0x205fd5f40..f4c`).
/// - `ltr_timestamp`: first present token for the frame is retained.
pub struct VideoFrameControlTracker<C> {
    pending: HashMap<(u32, u32), TrackedEntry<C>>,
    next_order: u64,
    max_entries: usize,
}

impl<C> Default for VideoFrameControlTracker<C> {
    fn default() -> Self {
        Self::with_capacity(MAX_TRACKED_FRAMES)
    }
}

impl<C> VideoFrameControlTracker<C> {
    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            pending: HashMap::new(),
            next_order: 0,
            max_entries,
        }
    }

    pub fn take(&mut self, ssrc: u32, timestamp: u32) -> Option<FrameControlEntry<C>> {
        let popped = self.pending.remove(&(ssrc, timestamp)).map(|e| e.control);
        self.pending.retain(|&(entry_ssrc, entry_ts), _| {
            entry_ssrc != ssrc || timestamp.wrapping_sub(entry_ts) >= 0x8000_0000
        });
        popped
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }
}

impl<C: Copy> VideoFrameControlTracker<C> {
    pub fn record(
        &mut self,
        ssrc: u32,
        timestamp: u32,
        ltr_bits: u8,
        ltr_timestamp: Option<u32>,
        camera_meta: Option<C>,
    ) {
        if let Some(existing) = self.pending.get_mut(&(ssrc, timestamp)) {
            if existing.control.camera_meta.is_none() {
                existing.control.camera_meta = camera_meta;
            }
            if existing.control.ltr_timestamp.is_none() {
                existing.control.ltr_timestamp = ltr_timestamp;
            }
            return;
        }

        if self.max_entries == 0 {
            return;
        }
        if self.pending.len() >= self.max_entries {
            if let Some(&oldest_key) = self
                .pending
                .iter()
                .min_by_key(|(_, entry)| entry.order)
                .map(|(key, _)| key)
            {
                self.pending.remove(&oldest_key);
            }
        }

        let order = self.next_order;
        self.next_order = self.next_order.wrapping_add(1);
        self.pending.insert(
            (ssrc, timestamp),
            TrackedEntry {
                control: FrameControlEntry {
                    ltr_bits,
                    ltr_timestamp,
                    camera_meta,
                },
                order,
            },
        );
    }
}
