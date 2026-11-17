//! Port of AVConference's 1:1 sender rate controller (iOS 27.0.1 24A446, iPhone17,5):
//! `VCRateControlAlgorithmLayeredContinuousTier` and the `VCRateControlAlgorithmStabilizedNOWRD`
//! / `VCRateControlAlgorithmBase` code it runs. Function for function from the decompiled
//! evidence in docs/apple-parity/ios27/evidence/sender-rate-20261116/ and
//! sender-rate-port-20261117/ (selection.md, config.md, ivars.md, port-notes.md).
//!
//! NOT wired into the call path. Time is always passed in; nothing here reads a clock.
//!
//! Status (chunk 3 of 4): state, inputs, media-controller interface,
//! `UpdateInternalStatistics`, ShouldRampDown / ShouldRampUp / ShouldFastRampUp,
//! IsBitrateOscillating, UpdateVCRateControlHistory and the Base/Util helpers they call.

pub mod base;
pub mod internal_stats;
pub mod media;
pub mod ramp_down;
pub mod ramp_up;
pub mod state;
pub mod stats;

pub use media::{MediaController, PacketLossInput, PacketLossOutput};
pub use state::VcrcState;
pub use stats::VcrcStatistics;
