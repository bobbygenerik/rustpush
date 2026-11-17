//! Port of AVConference's 1:1 sender rate controller (iOS 27.0.1 24A446, iPhone17,5):
//! `VCRateControlAlgorithmLayeredContinuousTier` and the `VCRateControlAlgorithmStabilizedNOWRD`
//! / `VCRateControlAlgorithmBase` code it runs. Function for function from the decompiled
//! evidence in docs/apple-parity/ios27/evidence/sender-rate-20261116/ and
//! sender-rate-port-20261117/ (selection.md, config.md, ivars.md, port-notes.md).
//!
//! NOT wired into the call path. Time is always passed in; nothing here reads a clock.
//!
//! Status (chunk 2 of 4): state, inputs, media-controller interface,
//! `UpdateInternalStatistics` and the Base/Util helpers it calls.

pub mod base;
pub mod internal_stats;
pub mod media;
pub mod state;
pub mod stats;

pub use media::{MediaController, PacketLossInput, PacketLossOutput};
pub use state::VcrcState;
pub use stats::VcrcStatistics;
