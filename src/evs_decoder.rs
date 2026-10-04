use std::ffi::c_void;
use std::ptr::NonNull;

extern "C" {
    fn obim_evs_create() -> *mut c_void;
    fn obim_evs_decode(state: *mut c_void, bytes: *const u8, bits: i32, pcm: *mut i16) -> i32;
    fn obim_evs_destroy(state: *mut c_void);
}

/// One decoder per participant/stream, used only while its owner mutex is held.
pub(super) struct EvsDecoder(NonNull<c_void>);
unsafe impl Send for EvsDecoder {}

impl EvsDecoder {
    pub(super) fn new() -> Option<Self> {
        NonNull::new(unsafe { obim_evs_create() }).map(Self)
    }

    pub(super) fn decode_payload(&mut self, payload: &[u8]) -> Option<Vec<u8>> {
        let frames = unpack_frames(payload)?;
        let mut decoded = Vec::with_capacity(frames.len() * 960);
        for (frame, bits) in frames {
            let mut pcm = [0i16; 960];
            if unsafe { obim_evs_decode(self.0.as_ptr(), frame.as_ptr(), bits, pcm.as_mut_ptr()) } != 960 {
                return None;
            }
            // 48k -> 24k, filtered decimation. This preserves the existing
            // 480-sample/20ms playback and echo-cancellation clock.
            for i in (0..960).step_by(2) {
                let at = |offset: isize| pcm[(i as isize + offset).clamp(0, 959) as usize] as i32;
                let sample = (-at(-3) + 9*at(-1) + 16*at(0) + 9*at(1) - at(3)) / 32;
                decoded.extend_from_slice(&(sample.clamp(-32768, 32767) as i16).to_le_bytes());
            }
        }
        Some(decoded)
    }
}
impl Drop for EvsDecoder {
    fn drop(&mut self) { unsafe { obim_evs_destroy(self.0.as_ptr()) } }
}

fn frame_bits(bytes: usize) -> Option<i32> {
    Some(match bytes {
        6 => 48, 15 => 118, 18 => 144, 20 => 160, 24 => 192,
        33 => 264, 41 => 328, 61 => 488, 80 => 640, 120 => 960,
        160 => 1280, 240 => 1920,
        _ => return None,
    })
}

fn unpack_frames(mut payload: &[u8]) -> Option<Vec<(&[u8], i32)>> {
    let mut frames = Vec::new();
    while !payload.is_empty() {
        let length = payload[0] as usize;
        let bits = frame_bits(length)?;
        if payload.len() < length + 1 { return None; }
        frames.push((&payload[1..length + 1], bits));
        payload = &payload[length + 1..];
    }
    if frames.is_empty() { None } else { Some(frames) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reference_encoded_24400_tone_decodes_at_existing_playback_clock() {
        // Generated with the TS 26.443 reference encoder: 1s of 440Hz,
        // 48kHz mono PCM, amplitude 8000, 24.4kbps primary mode. No call audio.
        let packet = include_bytes!("../third_party/evs-reference/test-tone-framed.bin");
        let pcm = EvsDecoder::new().unwrap().decode_payload(packet).unwrap();
        assert_eq!(pcm.len(), 48_000); // 1s, 24kHz, signed 16-bit mono.
        let samples: Vec<f64> = pcm[4_800..].chunks_exact(2)
            .map(|s| i16::from_le_bytes([s[0],s[1]]) as f64).collect();
        let rms = (samples.iter().map(|s| s*s).sum::<f64>() / samples.len() as f64).sqrt();
        assert!(rms > 3000.0 && rms < 8000.0, "decoded RMS {rms}");
        let crossings = samples.windows(2).filter(|s| s[0] <= 0.0 && s[1] > 0.0).count();
        let frequency = crossings as f64 * 24_000.0 / samples.len() as f64;
        assert!((frequency - 440.0).abs() < 10.0, "decoded frequency {frequency}");
    }
    #[test]
    fn rejects_truncation_and_unknown_rates_before_native_decode() {
        assert!(unpack_frames(&[61, 0]).is_none());
        assert!(unpack_frames(&[5, 0, 0, 0, 0, 0]).is_none());
        assert!(unpack_frames(&[]).is_none());
        let mut packet = vec![61]; packet.extend_from_slice(&[0; 61]);
        let frames = unpack_frames(&packet).unwrap();
        assert_eq!(frames.len(), 1); assert_eq!(frames[0].1, 488);
        packet.extend_from_slice(&[6, 0, 0, 0, 0, 0, 0]);
        assert_eq!(unpack_frames(&packet).unwrap().len(), 2);
    }
}
