//! Payload 104: raw AAC-ELD, 24 kHz mono, 480 samples (20 ms), without SBR.
//! MediaCodec's ELD encoder defaults to 512 samples and exposes no granule
//! setting. Use FDK explicitly rather than relabeling a different bitstream.
use fdk_aac_sys as fdk;
use std::{ffi::c_void, ptr};

pub const SAMPLES_PER_FRAME: usize = 480;
pub const PCM_BYTES_PER_FRAME: usize = SAMPLES_PER_FRAME * 2;
pub const CONFIGURATION: [u8; 4] = [0xf8, 0xec, 0x30, 0x00];

pub struct FaceTimeAacEncoder {
    handle: fdk::HANDLE_AACENCODER,
}

// Each handle is exclusively accessed through the session's audio_sender Mutex.
unsafe impl Send for FaceTimeAacEncoder {}

impl FaceTimeAacEncoder {
    pub fn new() -> Result<Self, String> {
        let mut encoder = Self { handle: ptr::null_mut() };
        check(unsafe { fdk::aacEncOpen(&mut encoder.handle, 0, 1) }, "open")?;
        for (parameter, value) in [
            (fdk::AACENC_PARAM_AACENC_AOT, 39),
            (fdk::AACENC_PARAM_AACENC_SAMPLERATE, 24_000),
            (fdk::AACENC_PARAM_AACENC_CHANNELMODE, 1),
            (fdk::AACENC_PARAM_AACENC_GRANULE_LENGTH, 480),
            (fdk::AACENC_PARAM_AACENC_SBR_MODE, 0),
            (fdk::AACENC_PARAM_AACENC_BITRATE, 32_000),
            (fdk::AACENC_PARAM_AACENC_TRANSMUX, 0),
        ] {
            check(unsafe { fdk::aacEncoder_SetParam(encoder.handle, parameter, value) }, "set parameter")?;
        }
        check(unsafe { fdk::aacEncEncode(encoder.handle, ptr::null(), ptr::null(), ptr::null(), ptr::null_mut()) }, "initialize")?;
        let mut info: fdk::AACENC_InfoStruct = unsafe { std::mem::zeroed() };
        check(unsafe { fdk::aacEncInfo(encoder.handle, &mut info) }, "info")?;
        if info.frameLength != 480 || info.inputChannels != 1
            || info.confSize != CONFIGURATION.len() as u32
            || info.confBuf[..CONFIGURATION.len()] != CONFIGURATION {
            return Err("AAC encoder did not produce ELD/24kHz/mono/480/no-SBR".into());
        }
        Ok(encoder)
    }

    pub fn encode(&mut self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        if bytes.len() != PCM_BYTES_PER_FRAME {
            return Err(format!("Expected {PCM_BYTES_PER_FRAME} bytes of PCM, got {}", bytes.len()));
        }
        let mut pcm: Vec<i16> = bytes.chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
        let mut output = vec![0u8; 1024];
        let mut input_ptr = pcm.as_mut_ptr() as *mut c_void;
        let mut input_id = fdk::AACENC_BufferIdentifier_IN_AUDIO_DATA as i32;
        let mut input_size = bytes.len() as i32;
        let mut input_element_size = 2;
        let input = fdk::AACENC_BufDesc {
            numBufs: 1, bufs: &mut input_ptr, bufferIdentifiers: &mut input_id,
            bufSizes: &mut input_size, bufElSizes: &mut input_element_size,
        };
        let mut output_ptr = output.as_mut_ptr() as *mut c_void;
        let mut output_id = fdk::AACENC_BufferIdentifier_OUT_BITSTREAM_DATA as i32;
        let mut output_size = output.len() as i32;
        let mut output_element_size = 1;
        let output_desc = fdk::AACENC_BufDesc {
            numBufs: 1, bufs: &mut output_ptr, bufferIdentifiers: &mut output_id,
            bufSizes: &mut output_size, bufElSizes: &mut output_element_size,
        };
        let input_args = fdk::AACENC_InArgs { numInSamples: SAMPLES_PER_FRAME as i32, numAncBytes: 0 };
        let mut output_args: fdk::AACENC_OutArgs = unsafe { std::mem::zeroed() };
        check(unsafe { fdk::aacEncEncode(self.handle, &input, &output_desc, &input_args, &mut output_args) }, "encode")?;
        if output_args.numInSamples != SAMPLES_PER_FRAME as i32 {
            return Err("AAC encoder did not consume the complete PCM frame".into());
        }
        output.truncate(output_args.numOutBytes as usize);
        Ok(output)
    }
}

impl Drop for FaceTimeAacEncoder {
    fn drop(&mut self) {
        if !self.handle.is_null() { unsafe { fdk::aacEncClose(&mut self.handle); } }
    }
}

fn check(status: fdk::AACENC_ERROR, operation: &str) -> Result<(), String> {
    if status == fdk::AACENC_ERROR_AACENC_OK { Ok(()) }
    else { Err(format!("AAC {operation} failed: 0x{status:x}")) }
}

/// AVConference audio payloads concatenate access units with one-byte sizes.
/// A raw access unit must never be interpreted as these frame-length headers.
pub fn frame_payload(access_unit: &[u8]) -> Result<Vec<u8>, String> {
    if access_unit.is_empty() || access_unit.len() > u8::MAX as usize {
        return Err(format!("AAC access unit cannot fit one-byte framing: {}B", access_unit.len()));
    }
    let mut payload = Vec::with_capacity(access_unit.len() + 1);
    payload.push(access_unit.len() as u8);
    payload.extend_from_slice(access_unit);
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_keeps_access_units_separate_and_rejects_oversize() {
        assert_eq!(frame_payload(&[0x97, 0x60]).unwrap(), [2, 0x97, 0x60]);
        assert!(frame_payload(&[]).is_err());
        assert!(frame_payload(&vec![0; 256]).is_err());
        assert_eq!(frame_payload(&vec![0; 255]).unwrap().len(), 256);
    }

    #[test]
    fn encoded_tone_decodes_as_480_samples_at_24khz() {
        let mut encoder = FaceTimeAacEncoder::new().unwrap();
        assert!(encoder.encode(&[0; 1024]).is_err());
        unsafe {
            let decoder = fdk::aacDecoder_Open(fdk::TRANSPORT_TYPE_TT_MP4_RAW, 1);
            assert!(!decoder.is_null());
            let mut config = CONFIGURATION;
            let mut config_ptr = config.as_mut_ptr();
            let config_size = config.len() as u32;
            assert_eq!(fdk::aacDecoder_ConfigRaw(decoder, &mut config_ptr, &config_size), 0);
            let mut pcm_out = [0i16; 480];
            let mut signal_energy = 0u64;
            let mut decoded = 0;
            for frame in 0..40 {
                let pcm: Vec<u8> = (0..480).flat_map(|i| {
                    let phase = (frame * 480 + i) as f64 * std::f64::consts::TAU * 1000.0 / 24000.0;
                    ((phase.sin() * 6000.0) as i16).to_le_bytes()
                }).collect();
                let mut au = encoder.encode(&pcm).unwrap();
                if au.is_empty() { continue; }
                assert!(frame_payload(&au).is_ok());
                let mut au_ptr = au.as_mut_ptr();
                let size = au.len() as u32;
                let mut valid = size;
                assert_eq!(fdk::aacDecoder_Fill(decoder, &mut au_ptr, &size, &mut valid), 0);
                assert_eq!(valid, 0);
                assert_eq!(fdk::aacDecoder_DecodeFrame(decoder, pcm_out.as_mut_ptr(), 480, 0), 0);
                let info = &*fdk::aacDecoder_GetStreamInfo(decoder);
                assert_eq!((info.sampleRate, info.numChannels, info.frameSize), (24000, 1, 480));
                if frame > 5 {
                    signal_energy += pcm_out.iter().map(|&v| (v as i64 * v as i64) as u64).sum::<u64>();
                }
                decoded += 1;
            }
            fdk::aacDecoder_Close(decoder);
            assert!(decoded > 30);
            assert!(signal_energy > 1_000_000_000, "tone was lost or decoded as silence");
        }
    }
}
