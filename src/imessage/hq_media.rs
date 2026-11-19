//! IPSW-verified iOS 27.0.1 (24A446) High-Quality Video / Photo constants and helpers.
//!
//! Sources in dyld_shared_cache_arm64e (24A446__iPhone17,5):
//! - `IDSFoundation`:
//!   - `_IDSRegistrationPropertySupportsHighQualityVideo` (`"supports-high-quality-video"`, `0x1e0c055c8`)
//!   - `_IDSRegistrationPropertySupportsHighQualityPhotoFileSizes` (`"supports-high-quality-photo-file-sizes"`, `0x1e0c05540`)
//! - `IMDaemonCore`:
//!   - `commonCapabilitiesSupportHighQualityPhotos:` (`0x2346d19a8`)
//!   - `commonCapabilitiesSupportHighQualityVideos:` (`0x2346d1a18`)
//!   - `mmcsTargetReportSizeForHighQualityPhotoSize:commonCapabilities:` (`0x2346d1b68`)
//!   - `mmcsTargetReportSizeForHighQualityVideoSize:commonCapabilities:` (`0x2346d1d4c`)
//!   - `_fetchStandardSizeLimit:highQualitySizeLimit:forTransfer:` (`0x2346d2278`)
//!   - `_smallerImageFileSize` (`0x2346d2900`)
//! - `IMFoundation`:
//!   - `_IMiMessageMaxTransferVideoFileSizeForWifi` (`0x19fd894a0`)
//!   - `_IMiMessageSizeLimitsForTransferType` (`0x19fd8a5dc`)

use std::collections::HashMap;

use crate::ids::user::ParsedClientData;

/// `IMDaemonCore` `modernHighQualityVideoSizeLimit` (`0x2346d18c0`..`0x2346d19a4`, default fallback `0x2346d1908`).
pub const MODERN_HIGH_QUALITY_VIDEO_SIZE_LIMIT: usize = 0x640_0000; // 104,857,600 B (100 MiB)

/// `IMDaemonCore` `modernHighQualityPhotoSizeLimit` (`0x2346d1700`..`0x2346d18b8`, default fallback `0x6400000`).
pub const MODERN_HIGH_QUALITY_PHOTO_SIZE_LIMIT: usize = 0x640_0000; // 104,857,600 B (100 MiB)

/// `IMFoundation` `_IMiMessageMaxTransferVideoFileSizeForWifi` (`0x19fd89518`).
pub const STANDARD_VIDEO_WIFI_SIZE_LIMIT: usize = 0xa0_0000; // 10,485,760 B (10 MiB)

/// `IMFoundation` `_IMiMessageMaxTransferVideoFileSizeForWifi` (`0x19fd89524`).
pub const STANDARD_VIDEO_CELLULAR_SIZE_LIMIT: usize = 0x40_0000; // 4,194,304 B (4 MiB)

/// `IMFoundation` `_IMiMessageMaxTransferFileSizeForWifi` (`0x19fd8932c`).
pub const STANDARD_PHOTO_WIFI_SIZE_LIMIT: usize = 0xa0_0000; // 10,485,760 B (10 MiB)

/// `IMFoundation` `_IMiMessageMaxTransferFileSizeForWifi` (`0x19fd89338`).
pub const STANDARD_PHOTO_CELLULAR_SIZE_LIMIT: usize = 0x40_0000; // 4,194,304 B (4 MiB)

/// `IMDaemonCore` `_smallerImageFileSize` (`0x2346d2900`).
pub const SMALLER_IMAGE_FILE_SIZE: usize = 0x7_d000; // 512,000 B

/// `IMDaemonCore` `tinyImageQualitySizeLimit` (`0x234734500`).
pub const TINY_IMAGE_QUALITY_SIZE_LIMIT: usize = 0x5_7800; // 358,400 B (350 KiB)

/// `IMDaemonCore.framework/MessageSendableUTIs.plist` exact allowlist.
pub const MESSAGE_SENDABLE_UTIS: &[&str] = &[
    "org.3gpp.adaptive-multi-rate-audio",
    "public.mp3",
    "com.apple.quicktime-movie",
    "com.apple.m4a-audio",
    "public.jpeg",
    "public.mpeg-4-audio",
    "public.png",
    "public.jpeg-2000",
    "public.camera-raw-image",
    "com.apple.coreaudio-format",
];

/// `IMDaemonCore` `mmcsTargetReportSizeForHighQualityVideoSize:commonCapabilities:` (`0x2346d1e14`).
pub const MMCS_HQ_VIDEO_REPORTED_SIZE_OVERRIDE: usize = 0x13f_ffff; // 20,971,519 B

/// `IMDaemonCore` `mmcsTargetReportSizeForHighQualityPhotoSize:commonCapabilities:` (`0x2346d1c30`).
pub const MMCS_HQ_PHOTO_REPORTED_SIZE_OVERRIDE: usize = 0x10_0000; // 1,048,576 B (1 MiB)

/// Result of checking IDS `client_data` capabilities across all chat recipients.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct CommonHqCapabilities {
    pub supports_high_quality_video: bool,
    pub supports_high_quality_photo_file_sizes: bool,
    pub supports_async_attachments: bool,
    pub supports_heif: bool,
}

/// Computes `commonCapabilities` for a set of recipients (`participant -> device client_data list`).
///
/// Mirrors `IMDaemonCore` `commonCapabilitiesSupportHighQualityVideos:` (`0x2346d1a18`) and
/// `commonCapabilitiesSupportHighQualityPhotos:` (`0x2346d19a8`): every participant must have
/// at least one resolved endpoint, and all resolved endpoints across recipients must advertise
/// the capability (`integerValue == 1`). Empty participant map or any recipient with zero resolved
/// identities fails closed (`false`).
pub fn compute_common_hq_capabilities(
    per_participant: &HashMap<String, Vec<ParsedClientData>>,
) -> CommonHqCapabilities {
    if per_participant.is_empty() {
        return CommonHqCapabilities::default();
    }

    let mut all_video = true;
    let mut all_photo = true;
    let mut all_async_attachments = true;
    let mut all_heif = true;

    for devices in per_participant.values() {
        if devices.is_empty() {
            return CommonHqCapabilities::default();
        }
        if !devices.iter().all(|d| d.supports_high_quality_video) {
            all_video = false;
        }
        if !devices
            .iter()
            .all(|d| d.supports_high_quality_photo_file_sizes)
        {
            all_photo = false;
        }
        if !devices.iter().all(|d| d.supports_async_attachments) {
            all_async_attachments = false;
        }
        if !devices.iter().all(|d| d.supports_heif) {
            all_heif = false;
        }
    }

    CommonHqCapabilities {
        supports_high_quality_video: all_video,
        supports_high_quality_photo_file_sizes: all_photo,
        supports_async_attachments: all_async_attachments,
        supports_heif: all_heif,
    }
}

/// Ports `IMDaemonCore` `mmcsTargetReportSizeForHighQualityVideoSize:commonCapabilities:` (`0x2346d1d4c`).
pub fn mmcs_target_report_size_for_hq_video(
    actual_size: usize,
    recipients_support_hqv: bool,
    hqv_enabled: bool,
    allow_compat_override: bool,
    bag_override_cap: Option<usize>,
) -> usize {
    if !recipients_support_hqv
        && hqv_enabled
        && allow_compat_override
        && actual_size <= MODERN_HIGH_QUALITY_VIDEO_SIZE_LIMIT
    {
        let cap = bag_override_cap.unwrap_or(MMCS_HQ_VIDEO_REPORTED_SIZE_OVERRIDE);
        actual_size.min(cap)
    } else {
        actual_size
    }
}

/// Ports `IMDaemonCore` `mmcsTargetReportSizeForHighQualityPhotoSize:commonCapabilities:` (`0x2346d1b68`).
pub fn mmcs_target_report_size_for_hq_photo(
    actual_size: usize,
    recipients_support_hqp: bool,
    hqp_enabled: bool,
    allow_compat_override: bool,
    bag_override_cap: Option<usize>,
) -> usize {
    if !recipients_support_hqp
        && hqp_enabled
        && allow_compat_override
        && actual_size <= MODERN_HIGH_QUALITY_PHOTO_SIZE_LIMIT
    {
        let cap = bag_override_cap.unwrap_or(MMCS_HQ_PHOTO_REPORTED_SIZE_OVERRIDE);
        actual_size.min(cap)
    } else {
        actual_size
    }
}
