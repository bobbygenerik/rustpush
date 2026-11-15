//! Feedback delay fields matching Apple AVConference (`iOS 27.0.1 24A446`).
//!
//! - `pack_queuing_delay`: `_RTPSetAFRCProcessingDelay` (`0x205fa576c` at `0x205fa57e0..0x205fa57fc`).
//! - `ntp_send_timestamp`: `_TimeZeroInit` (`0x205f901a0`), `_MicroToNTP` (`0x205f90154`),
//!   `_NTPToMiddle32` (`0x205f902c8`), and `_RTPSetAFRCProcessingDelay` (`0x205fa5800..0x205fa5814`).

/// Seconds between NTP epoch (`1900-01-01T00:00:00Z`) and Unix epoch (`1970-01-01T00:00:00Z`),
/// from `_TimeZeroInit` constant at `0x2066a5b98` (`2208988800.0`).
pub const NTP_UNIX_EPOCH_OFFSET_SECS: f64 = 2_208_988_800.0;

/// Scale factor `2^32` from seconds to 32.32 NTP fixed-point (`_MicroToNTP` `0x205f90184`).
const NTP_FRAC_SCALE: f64 = 4_294_967_296.0;

/// Compute `queuingDelay` in `1/1024 s` ticks from monotonic `enqueue_time_s` and `send_time_s` (seconds),
/// matching `_RTPSetAFRCProcessingDelay` (`0x205fa57e0..0x205fa57fc`) clamped to `0xffff` on the wire.
pub fn pack_queuing_delay(enqueue_time_s: f64, send_time_s: f64) -> u16 {
    if enqueue_time_s == 0.0 {
        return 0;
    }
    let delta_s = (send_time_s - enqueue_time_s).max(0.0);
    let ticks = (delta_s * 1024.0) as u32;
    ticks.max(1).min(u16::MAX as u32) as u16
}

/// Convert Unix-epoch seconds (`duration_since_epoch().as_secs_f64()`) to Apple's `sendTimestamp`
/// (`(MicroToNTP(send_time_s) >> 22) & 0xffff`), in `1/1024 s` (`1024 Hz`) ticks wrapping every 64 s.
pub fn ntp_send_timestamp(unix_time_s: f64) -> u16 {
    let ntp_s = unix_time_s + NTP_UNIX_EPOCH_OFFSET_SECS;
    let ntp64 = (ntp_s * NTP_FRAC_SCALE) as u64;
    let middle32 = (ntp64 >> 16) as u32;
    (middle32 >> 6) as u16
}
