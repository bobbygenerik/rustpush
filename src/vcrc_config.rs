//! Data extracted from AVConference, iOS 27.0.1 24A446, iPhone17,5.
//!
//! Later-write audit covers the mode-1 Wi-Fi camera profile with no overrides.
//! Four runtime-dependent fields remain in EFFECTIVE_VALUE_UNKNOWNS (0x186 and 0x18e resolved later); their values
//! retain the documented binary-default fallback. Other values have had their
//! later stores and the two Configure copy paths checked for this profile.
//! This static data audit does not establish live-call or full controller parity.
//! A live learnt-bitrate delegate may replace the cold initial tier 17.
//! No controller is wired to these values by this data-only module.

/// `_vcRateControlTierBitrates`, 66 little-endian u32 words at 0x2066a3b10.
/// Every word was read from the cache and checked against its raw dump.
pub const TIER_BITRATES: [u32; 66] = [
    10000, 15000, 20000, 36000, 52000, 68000,
    84000, 100000, 116000, 132000, 164000, 196000,
    228000, 260000, 299000, 355000, 422000, 502000,
    597000, 692000, 802000, 930000, 1078000, 1250000,
    1450000, 1681000, 1949000, 2260000, 2620000, 3038000,
    3522000, 4084000, 4735000, 5490000, 6365000, 7380000,
    8557000, 9920000, 11500000, 13332000, 15455000, 17917000,
    20771000, 24080000, 27915000, 32362000, 37517000, 43493000,
    50421000, 58448000, 67757000, 78550000, 91062000, 103574000,
    116086000, 128598000, 141110000, 153622000, 166134000, 178646000,
    191158000, 203670000, 216182000, 228694000, 241206000, 253718000,
];

/// Config offsets whose final effective value remains UNKNOWN.
pub const EFFECTIVE_VALUE_UNKNOWNS: &[u16] = &[
    0x10, 0x54, 0x370, 0x428,
];

/// Named by native config-relative offset; this is not a C ABI memory layout.
#[derive(Clone, Debug, PartialEq)]
pub struct VcrcConfig {
    /// Config +0x8; _vcRateControlTierBitrates.
    /// 0x205eaa578..580: page + 0xb10, store at sp+0x50.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Native pointer 0x2066a3b10 is represented by a host reference to the extracted table.
    pub o_8: &'static [u32; 66],
    /// Config +0x0: the caller's rate-control mode. ConfigureInternal copies `[x1]` into
    /// controller+0x48 (0x205eaa18c..19c) and then into sp+0x48 = config+0 (0x205eaa56c..574).
    /// Value 1 is ASSUMED for a 1:1 camera call: selection.md reads the mode at controller+0x58,
    /// not +0x48, and the two have not been reconciled. Only read by IsVideoStopped
    /// 0x20617a9e8 (modes 1, 2, 7 consult the media controller).
    pub o_0: u32,
    /// Config +0x10; initial tier (role established by table lookup at 0x205eaceb4..ec0).
    /// 0x205eaa5fc..614 selects 17 when max>25; 0x205eaa634..790 learnt path; 0x205eaa87c..890 clamp/store at sp+0x58.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Effective value UNKNOWN; initializer uses an inspected builder default or binary zero.
    /// Binary cold default 17; learnt delegate and forcedInitialBitrate can change the live tier.
    /// Runtime learnt-bitrate delegate remains UNKNOWN (0x205eaa634..790); cold fallback 17.
    pub o_10: i32,
    /// Config +0x14; max tier (clamp use 0x2061fb94c..958).
    /// 0x205eaa584..5c0 chooses upper tier 31/42/16 according to traffic and feature bits 16/19 and scans down; 0x205eaa5cc..5d0 stores nonnegative result at sp+0x5c.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    pub o_14: i32,
    /// Config +0x18; min tier (0x2061fb9b4..9bc passes table[min] to SetMinTargetBitrate).
    /// 0x205eb0708, 0x205eaa4b0, 0x205eaa5d4..5f8; table is already >=0 at tier 0.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    pub o_18: i32,
    /// Config +0x1c; forcedSoftMaxTierVeryLowIndex.
    /// 0x205eaa898..8a8: default w1=7; store at sp+0x64.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa8a8.
    pub o_1c: i32,
    /// Config +0x20; forcedSoftMaxTierLowIndex.
    /// 0x205eaa8b0..8c0: default w1=0x12; store at sp+0x68.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa8c0, 0x205eaadf8.
    pub o_20: i32,
    /// Config +0x28; forcedSoftMaxTierMediumIndex.
    /// 0x205eaa8c8..8d4: default w1=0x1a; result survives until store at 0x205eaa9b8, sp+0x70.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa9b8.
    pub o_28: i32,
    /// Config +0x2c; native role UNKNOWN.
    /// Effective 0: config zero at 0x205eaa560..568; +0x2c stores at
    /// 0x205eaaef4/0x205eab2fc are on excluded RAT/mode paths.
    /// forcedSoftMaxTierMildLowIndex at 0x205eaac04 is +0x24, not this field.
    /// No post-copy write in 0x2061fb4b4..bba0 or 0x205f95c34..cd4.
    pub o_2c: i32,
    /// Config +0x30.
    /// 0x205eaa9b4..9bc: w12=3, store at sp+0x78.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa9bc, 0x205eab8b4.
    pub o_30: i32,
    /// Config +0x3c.
    /// 0x205eaa8d8..8dc: movi v0.4s,#1; q0 stored at sp+0x84, lane 0.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa8dc.
    pub o_3c: i32,
    /// Config +0x40.
    /// Same vector store 0x205eaa8d8..8dc, lane 1.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa8dc.
    pub o_40: i32,
    /// Config +0x44.
    /// Same vector store 0x205eaa8d8..8dc, lane 2.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa8dc, 0x205eab000, 0x205eab1fc, 0x205ead4a4, 0x205ead4ec.
    pub o_44: i32,
    /// Config +0x48.
    /// Same vector store 0x205eaa8d8..8dc, lane 3.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa8dc, 0x205ead4a4.
    pub o_48: i32,
    /// Config +0x4c.
    /// Zero initialization; 0x205eaab24 writes sp+0x94 in a VCDefaults-object conditional branch.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: B branch overwrites the initial store but the prior exported value already reflects it. No subsequent write on the stated path; no post-copy write.
    /// Potential writers: 0x205eaab24, 0x205eaaffc, 0x205eab164, 0x205eac400, 0x205ead4f4.
    pub o_4c: u32,
    /// Config +0x50.
    /// Zero initialization; paired word at sp+0x98 is written by 0x205eaab24; mode/RAT store at 0x205eab148.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: B branch overwrites the initial store but the prior exported value already reflects it. No subsequent write on the stated path; no post-copy write.
    /// Potential writers: 0x205eaab24, 0x205eab148, 0x205eac420.
    pub o_50: u32,
    /// Config +0x54.
    /// 0x205eaab94..9c: lsl by 16 then arithmetic sign extraction and AND 17; 0x205eaabb0 stores at sp+0x9c.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Effective value UNKNOWN; initializer uses an inspected builder default or binary zero.
    /// Runtime isUsingPersonaCamera input remains UNKNOWN (0x206430f3c..4c); fallback 0.
    pub o_54: u32,
    /// Config +0x58.
    /// Zero initialization covers sp+0xa0; later-writer audit outstanding.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205ead4b0.
    pub o_58: u32,
    /// Config +0x68; NOWRDAcc (ServerBag key).
    /// q0 low/high constants at 0x2066a3970/978 loaded by 0x205eaa8e4; q0 stored at sp+0xa8 by 0x205eaa8f0, high lane at sp+0xb0.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa8f0, 0x205eab1e8, 0x205eab238, 0x205eac210.
    pub o_68: f64,
    /// Config +0x60 (ServerBag key "rampDown-delay"/"NOWRD" at 0x205eac1f0): NOWRD threshold
    /// used when send-limited. 0x205eaa8e4/8f0: q0 from 0x2066a3970 = (0.1, 0.15) stored at
    /// sp+0xa8 (config+0x60 low lane, +0x68 high lane). Other writers 0x205eab1e8/0x205eab238
    /// (X-RAT doubling) and 0x205eac1f0 (S) are excluded exactly as for o_68 in config.md.
    pub o_60: f64,
    /// Config +0x70 ("aggNOWRD", 0x205eac230): NOWRD threshold when not send-limited.
    /// 0x205eaa8ec/8f4: q1 from 0x2066a3980 = (0.06, 0.1) stored at sp+0xb8.
    pub o_70: f64,
    /// Config +0x78 ("aggNOWRDAcc", 0x205eac250): accumulated-NOWRD threshold during
    /// initial ramp / settling when not send-limited. High lane of the same q1: 0.1.
    pub o_78: f64,
    /// Config +0x1a0: multiplier on the NOWRD-accumulation threshold while ramp-down is
    /// suppressed (ShouldRampDown 0x2065c1f24). Only writer is 0x205eac2d0 inside the
    /// ServerBag group skipped at 0x205eac1c8 (S), so it stays 0 from the bzero
    /// at 0x205eaa560..568. Unused on the static profile because o_194 == 0.
    pub o_1a0: f64,
    /// Config +0x80.
    /// 0x205eaa8f8..8fc: bits 0x4000000000000000, store at sp+0xc8.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa8fc, 0x205eac270.
    pub o_80: f64,
    /// Config +0x88; keepOvershootDuration.
    /// 0x205eaa900..914: test byte at controller+0x60, select 6 if nonzero else 2, store at sp+0xd0.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa914, 0x205eac588.
    pub o_88: f64,
    /// Config +0x90; nextTierPercentage.
    /// Load q1 from 0x2066a39a0 at 0x205eaa924, low lane; store at sp+0xd8 at 0x205eaa92c.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa92c, 0x205eac5a8.
    pub o_90: f64,
    /// Config +0x98.
    /// High lane of same q1 load/store 0x205eaa924..92c, constant 0x2066a39a8.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa92c, 0x205eab1b4, 0x205ead4dc.
    pub o_98: f64,
    /// Config +0xa0.
    /// 0x205eaa928..930: vector immediate 2.0 in both f64 lanes; store at sp+0xe8, low lane.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa930, 0x205eab1b4, 0x205ead4dc.
    pub o_a0: f64,
    /// Config +0xa8.
    /// Same vector immediate/store 0x205eaa928..930, high lane at sp+0xf0.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa930, 0x205eaae50, 0x205eabf88.
    pub o_a8: f64,
    /// Config +0xb0.
    /// q0 from 0x2066a3990 at 0x205eaa91c; low lane stored at sp+0xf8 by 0x205eaa94c.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa94c, 0x205eabfa8.
    pub o_b0: f64,
    /// Config +0xc0.
    /// q1 from 0x2066a39b0 at 0x205eaa948; store at sp+0x108 via x8=sp+9, stur q1,[x8,#0xff] at 0x205eaa950..954, low lane.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa954, 0x205eac0b8.
    pub o_c0: f64,
    /// Config +0xc8.
    /// Same q1 store at 0x205eaa954, high lane from 0x2066a39b8, sp+0x110.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa954, 0x205eabd14.
    pub o_c8: f64,
    /// Config +0xd8.
    /// 0x205eaa934..938: w10=1, stored at sp+0x120.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa938, 0x205eac0d8.
    pub o_d8: i32,
    /// Config +0xdc.
    /// Zero initialization; 0x205eaab4c stores eight bytes beginning at config+0xdc in a VCDefaults-object conditional path.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: B branch overwrites the initial store but the prior exported value already reflects it. No subsequent write on the stated path; no post-copy write.
    /// Potential writers: 0x205eaab4c, 0x205eaae58, 0x205eac440.
    pub o_dc: u32,
    /// Config +0xe0.
    /// Zero initialization; same overlapping eight-byte write at 0x205eaab4c; RAT-specific store at 0x205eab14c.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: B branch overwrites the initial store but the prior exported value already reflects it. No subsequent write on the stated path; no post-copy write.
    /// Potential writers: 0x205eaab4c, 0x205eab14c.
    pub o_e0: u32,
    /// Config +0xe8.
    /// Zero initialization; conditional floating store at sp+0x130, 0x205eaab44.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: B branch overwrites the initial store but the prior exported value already reflects it. No subsequent write on the stated path; no post-copy write.
    /// Potential writers: 0x205eaab44, 0x205eac6d0.
    pub o_e8: f64,
    /// Config +0xf0.
    /// Zero initialization; q0 store starting at sp+0x138 by 0x205eaab5c in the VCDefaults-object conditional path.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: B branch overwrites the initial store but the prior exported value already reflects it. No subsequent write on the stated path; no post-copy write.
    /// Potential writers: 0x205eaab5c.
    pub o_f0: f64,
    /// Config +0xf8; lossEventNOWRD (ServerBag key).
    /// Zero initialization; high lane of same conditional q0 store 0x205eaab5c.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: B branch overwrites the initial store but the prior exported value already reflects it. No subsequent write on the stated path; no post-copy write.
    /// Potential writers: 0x205eaab5c, 0x205eac460.
    pub o_f8: f64,
    /// Config +0x108.
    /// 0x205eaa93c..940 stores bits 0x3fe8000000000000 at sp+0x150.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa940, 0x205eab008, 0x205eabcf4.
    pub o_108: f64,
    /// Config +0x110: send-limited duration read by StabilizedNOWRD UpdateInternalStatistics.
    /// 0x205eaa95c..96c: q0 from 0x2066a39c0 (1.0; high lane 0x2066a39c8 = 2.0) stored by
    /// `stur q0, [x8, #0xff]` with x8 = sp+0x59 (0x205eaa968), i.e. sp+0x158 = config+0x110.
    /// later-write-stores.tsv lists 0x205eaa96c as the only writer of +0x110.
    pub o_110: f64,
    /// Config +0x118.
    /// 0x205eaa95c..96c: q0 from 0x2066a39c0, high lane at 0x2066a39c8, stored at sp+0x160.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa96c.
    pub o_118: f64,
    /// Config +0x120.
    /// 0x205eaa964..974: q1 from 0x2066a39d0, low lane stored at sp+0x168.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa974, 0x205eab1d4, 0x205eab224.
    pub o_120: f64,
    /// Config +0x128.
    /// Same load/store, high lane from 0x2066a39d8, sp+0x170.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa974, 0x205eab1d4, 0x205eab224.
    pub o_128: f64,
    /// Config +0x130.
    /// 0x205eaa97c..98c: q0 from 0x2066a39e0, low lane stored at sp+0x178.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa98c.
    pub o_130: f64,
    /// Config +0x138.
    /// Same load/store, high lane from 0x2066a39e8, sp+0x180.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa98c.
    pub o_138: f64,
    /// Config +0x140.
    /// 0x205eaa984..994: q1 from 0x2066a39f0, first u32 lane at sp+0x188; bytes 32 00 00 00.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa994, 0x205eab1c0, 0x205eab210.
    pub o_140: u32,
    /// Config +0x144.
    /// Same q1 load/store, second u32 lane from 0x2066a39f4, sp+0x18c.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa994, 0x205eab1c0, 0x205eab210.
    pub o_144: u32,
    /// Config +0x148.
    /// Same q1 load/store, third u32 lane from 0x2066a39f8, sp+0x190; bytes 00 7d 00 00.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa994, 0x205ead4a8.
    pub o_148: u32,
    /// Config +0x14c.
    /// Same q1 load/store, fourth u32 lane from 0x2066a39fc, sp+0x194.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa994, 0x205eaae68, 0x205eab1f4, 0x205ead4a8, 0x205ead4f0.
    pub o_14c: u32,
    /// Config +0x150.
    /// 0x205eaa998..9a0 selects w24=7 or w8=4 using the remote-cellular comparison at 0x205eaa904; stores at sp+0x198.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa9a0.
    pub o_150: u32,
    /// Config +0x158.
    /// Config array base formed at 0x205eaabc0; index-0 store 0x205eaabc8..bd0; helper 0x2060de4b4..4cc loads constant at 0x2066a76b0.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaabd0.
    pub o_158: f64,
    /// Config +0x160.
    /// Config array index 1 stored at 0x205eaabd0; helper 0x2060de4b4..4cc reads 0x2066a76b8, bytes 00 00 00 00 00 00 e8 3f.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaabd0.
    pub o_160: f64,
    /// Config +0x168.
    /// Same loop, index 2; helper reads 0x2066a76c0, bytes 00 00 00 00 00 00 f8 3f.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaabd0, 0x205eb1bc4, 0x205eb1b9c.
    pub o_168: f64,
    /// Config +0x170.
    /// Same loop, index 3; 0x2060de4b4 sets d0=5, comparison at 0x2060de4b8..4bc skips the table for index >2.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaabd0, 0x205eb1be4, 0x205eb1b9c.
    pub o_170: f64,
    /// Config +0x178.
    /// Same loop, index 4, same immediate/branch; loop bound is 5 at 0x205eaabd8.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaabd0, 0x205eb1c04, 0x205eb1b9c.
    pub o_178: f64,
    /// Config +0x182.
    /// 0x205eaa9a8..9b0, store at sp+0x1ca.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa9b0, 0x205eab1f0, 0x205eab240, 0x205ead4e0.
    pub o_182: u8,
    /// Config +0x183.
    /// 0x205eaa9dc..9e8 writes halfword 0x0100 at config+0x183, low byte 0.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa9e8, 0x205eaaec4, 0x205eab0dc, 0x205eab2c0, 0x205eb12c0, 0x205eb1290.
    pub o_183: u8,
    /// Config +0x180 ("enableBytesInFlightAdaptation", VCDefaults key at 0x205ead1e0):
    /// bit(F,14) stored by 0x205eaabb4..bb8 (the same bit as +0x181, the continuous-tier
    /// selector, which selection.md establishes as 1). VCDefaults keeps it with no override
    /// (0x205ead1d4..1e8). 0x205ead4b4 zeroes it only on another mode path. Copied to
    /// `_bytesInFlightAdaptationEnabled` by StabilizedNOWRD Configure.
    pub o_180: u8,
    /// Config +0x18b: RampDownSpeedFactor 0x205f97440 uses 1.0 instead of 3.0 when set.
    /// 0x205eaa900/0x205eaa9c8: the controller+0x60 byte (the same flag that makes +0x88
    /// 6.0 instead of 2.0). 0 on this Wi-Fi profile, consistent with o_88 = 2.0.
    pub o_18b: u8,
    /// Config +0x200: GetBitrateOscillationCount 0x2065c18b8 threshold factor
    /// (× tier[o_14]). 0x205eaaa2c..a38 stores bits 0x3fe4cccccccccccd at sp+0x248.
    pub o_200: f64,
    /// Config +0x1f0 ("durationRatio") / +0x1f8 ("aggDurationRatio"): StabilizedNOWRD
    /// StateEnter(4) multipliers. q0 from 0x2066a3a10 = (4.0, 8.0) stored by 0x205eaaa28
    /// (x9 = sp+0x139, +0xff). Other writers 0x205eab718/738 are ServerBag (S).
    pub o_1f0: f64,
    /// See o_1f0.
    pub o_1f8: f64,
    /// Config +0x185.
    /// 0x205eaaa00..a04, sp+0x1cd.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaa04.
    pub o_185: u8,
    /// Config +0x186.
    /// 0x205eaaa08..a0c, sp+0x1ce.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// RESOLVED (2026-11-17, Claude) = 0: bit 7 of the default flag word from
    /// -[AVCRateController loadDefaultVCRCFeatureFlags:] 0x205eb0a98, which is 0xbd only when
    /// the VCDefaults bool at 0x2721ecba0 is true (0x205eb0ccc..cdc), else 0x3d. That bool
    /// defaults to 0 unless VCFeatureFlagManager_ReduceKPIVariation 0x20643c010 is true
    /// (0x205eb0ac0..b20), and that needs os_feature_enabled(AVConference/ReduceKPIVariation)
    /// (block 0x20643c13c); the 24A446 FeatureFlags plist has ReduceKPIVariation with only
    /// DevelopmentPhase=FeatureComplete and no Enabled key, i.e. off on customer builds.
    /// The same 0x3d word confirms +0x185 (bit 6) = 0, +0x187 (bit 3) = 1, +0x188 (bit 5) = 1.
    pub o_186: u8,
    /// Config +0x187.
    /// 0x205eaaa90..ac4, byte 0 of four-byte feature extraction store; shift constants directly dumped at 0x2066a3908..917.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaac4, 0x205eab528.
    pub o_187: u8,
    /// Config +0x188.
    /// 0x205eaaa90..ac4: vector shifts from 0x2066a3908 (-3,-5) and 0x2066a3910 (-11,-8), halfword/byte unzip and mask 1, byte 1 of store at config+0x187.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaac4, 0x205eabb3c.
    pub o_188: u8,
    /// Config +0x189.
    /// Same vector sequence, byte 2.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaac4.
    pub o_189: u8,
    /// Config +0x18a.
    /// Same vector sequence, byte 3.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaac4, 0x205eab0c0, 0x205eab874.
    pub o_18a: u8,
    /// Config +0x18e.
    /// 0x205eaa560..568 zeros config+0..0x42f.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// RESOLVED (2026-11-17, Claude) = 1 ("rateControlLowBandwidthOptimization"):
    /// 0x205ead200..210 stores VCDefaults_GetBoolValueForKey(key, controller+0x7e0) here.
    /// controller+0x7e0 (`_lowBandwidthOptimizationEnabled`) is set to 1 at 0x205ea80d4..d8 in
    /// -[AVCRateController loadParametersFromExperimentManagerForABTesting]; the only other
    /// writer is the experiment-manager loader at 0x205ea8490..4a4
    /// (kVCExperimentFTLowBandwidthOptimization), the same mechanism that leaves
    /// `_abTestRateRampUp` (+0x701) at its default 1 in selection.md.
    pub o_18e: u8,
    /// Config +0x194.
    /// 0x205eaaa6c..a70, sp+0x1dc.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaa70, 0x205eac290.
    pub o_194: u8,
    /// Config +0x198.
    /// x22=sp+0x1cf from 0x205eaa558; 0x205eaaa78..a7c stores q0 from 0x2066a3a40 at x22+0x11=sp+0x1e0, low lane.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaa7c, 0x205eac2b0.
    pub o_198: f64,
    /// Config +0x1a8.
    /// w25=0x1a at 0x205eaa8cc; stored at sp+0x1f0 by 0x205eaaa3c.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaa3c.
    pub o_1a8: u32,
    /// Config +0x1b0.
    /// 0x205eaaa44..a48: bits 0x4034000000000000 stored at sp+0x1f8.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaa48, 0x205eab588.
    pub o_1b0: f64,
    /// Config +0x1b8 ("minTierGap", ServerBag key at 0x205eab550): tiers above the current one
    /// that the remote BWE must clear for a fast ramp-up (ShouldFastRampUp 0x2065c1a78).
    /// `mov w12, #0x3` at 0x205eaa9b4, stored at sp+0x200 by 0x205eaaa40. The only other
    /// writer, 0x205eab568, is inside the ServerBag group (S).
    pub o_1b8: u32,
    /// Config +0x1c0.
    /// 0x205eaaacc..ad4: q0 from 0x2066a3a50, low lane stored at sp+0x208 via sp+0x109+0xff.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaad4, 0x205eab548.
    pub o_1c0: f64,
    /// Config +0x1c8.
    /// Same load/store, high lane from 0x2066a3a58, sp+0x210.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaad4, 0x205eabb5c.
    pub o_1c8: f64,
    /// Config +0x1d0.
    /// x8 set to bits 0x4008000000000000 at 0x205eaa9c0; retained until store at sp+0x218 by 0x205eaaad8.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaad8, 0x205eabb7c.
    pub o_1d0: f64,
    /// Config +0x1d8.
    /// 0x205eaaadc..ae0, w8=5, sp+0x220.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaae0, 0x205eabb9c.
    pub o_1d8: u32,
    /// Config +0x1e0; native role UNKNOWN.
    /// Effective 0.002: 0x205eaaae4..af4 constructs bits 0x3f60624dd2f1a9fc
    /// and stores at sp+0x228. The 0.65 store at 0x205eaaa38 is +0x200.
    /// Later 0x205eabbbc is ServerBag-only; no post-copy write.
    pub o_1e0: f64,
    /// Config +0x1e8.
    /// 0x205eaaa14..a18 copies 8 constant bytes from 0x2066a3900 to sp+0x230; low word bytes 04 00 00 00.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaa18, 0x205eab6f8.
    pub o_1e8: u32,
    /// Config +0x1ec.
    /// Same 8-byte copy, high word from 0x2066a3904, bytes 03 00 00 00.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaa18, 0x205eab6d8.
    pub o_1ec: u32,
    /// Config +0x208.
    /// w10=1 at 0x205eaa934; stored at sp+0x250 by 0x205eaa9d4; alternative-mode write at 0x205eab0b8 is not yet fully excluded.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa9d4, 0x205eab0b8.
    pub o_208: u32,
    /// Config +0x238.
    /// Zero initialization at sp+0x280; later-writer audit outstanding.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaf0c, 0x205eab30c.
    pub o_238: f64,
    /// Config +0x240.
    /// 0x205eaa9c0..9c4: bits 0x4008000000000000 stored at sp+0x288.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa9c4, 0x205eab894.
    pub o_240: f64,
    /// Config +0x268.
    /// Zero initialization at sp+0x2b0; RAT-specific write at 0x205eab108.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaee0, 0x205eab108, 0x205eab2e4, 0x205eac8e8.
    pub o_268: f64,
    /// Config +0x270.
    /// Zero initialization at sp+0x2b8; RAT-specific q0 store at 0x205eab11c.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaeec, 0x205eab11c, 0x205eab2f0, 0x205eac8c8.
    pub o_270: f64,
    /// Config +0x278.
    /// Same zero initialization; high lane of RAT-specific q0 store at 0x205eab11c.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaeec, 0x205eab11c, 0x205eab2f0, 0x205eac908.
    pub o_278: f64,
    /// Config +0x288.
    /// Zero initialization at sp+0x2d0; RAT-specific q1 store at 0x205eab124.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eab124.
    pub o_288: f64,
    /// Config +0x290.
    /// Zero initialization at sp+0x2d8; RAT-specific immediate store at 0x205eab128.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eab128.
    pub o_290: f64,
    /// Config +0x298; networkUnstableCoolDownTime.
    /// Zero at 0x205eaa560..568, sp+0x2e0.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaa58, 0x205eabe28.
    pub o_298: f64,
    /// Config +0x2a0; native role UNKNOWN.
    /// Effective 10.0: 0x205eaaa4c..a58 stores q0 through config+0x298;
    /// high lane comes from 0x2066a3a28 (bits 0x4024000000000000).
    /// Later 0x205eaba18 is ServerBag-only; 0x205eaae70 is another mode.
    /// No post-copy write in 0x2061fb4b4..bba0 or 0x205f95c34..cd4.
    pub o_2a0: f64,
    /// Config +0x2d0.
    /// Explicit strb wzr,[sp,#0x318] at 0x205eaa9ec.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaa9ec, 0x205eab160.
    pub o_2d0: u8,
    /// Config +0x2d4.
    /// 0x205eaa560..568; conditional later store at 0x205eab174.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eab174.
    pub o_2d4: u32,
    /// Config +0x2d8; RTTThreshold in the noRampUp-network ServerBag group.
    /// Effective 0.5: 0x205eaaa5c..a68 stores q0 through config+0x2d8;
    /// low lane comes from 0x2066a3a30 (bits 0x3fe0000000000000).
    /// Later 0x205eabe48 is ServerBag-only; no post-copy write.
    pub o_2d8: f64,
    /// Config +0x2e0; PLRThreshold.
    /// Same zero, sp+0x328.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaa68, 0x205eabe68.
    pub o_2e0: f64,
    /// Config +0x2e8.
    /// 0x205eaab64..6c, sp+0x330.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaab6c, 0x205eb1094, 0x205eb106c.
    pub o_2e8: f64,
    /// Config +0x2f0.
    /// 0x205eaab74 loads q0 from 0x2066a3a70; 0x205eaaba0..ba4 stores at sp+0x338, low lane.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaba4, 0x205eb10b4, 0x205eb106c.
    pub o_2f0: f64,
    /// Config +0x2f8.
    /// Same q0 store, high lane from 0x2066a3a78 at sp+0x340.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaaba4, 0x205eb10d4, 0x205eb106c.
    pub o_2f8: f64,
    /// Config +0x300.
    /// 0x205eaab7c loads q1 from 0x2066a3a80; 0x205eaaba8..bac stores at sp+0x348, low lane.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaabac, 0x205eb10f4, 0x205eb106c.
    pub o_300: f64,
    /// Config +0x308.
    /// Same q1 store, high lane from 0x2066a3a88 at sp+0x350.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaabac, 0x205eb1114, 0x205eb106c.
    pub o_308: f64,
    /// Config +0x310.
    /// 0x205eaab80..b90 assembles bits 0x3f9eb851eb851eb8 and stores at sp+0x358.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaab90, 0x205eb1134, 0x205eb106c.
    pub o_310: f64,
    /// Config +0x330.
    /// 0x205eaac0c..c14: q0 from 0x2066a3a90, low lane stored at sp+0x378.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaac14.
    pub o_330: f64,
    /// Config +0x338.
    /// Same q0 store, high lane from 0x2066a3a98, sp+0x380.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaac14.
    pub o_338: f64,
    /// Config +0x340.
    /// 0x205eaac18..c24: bits 0x3fb999999999999a at sp+0x388.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaac24.
    pub o_340: f64,
    /// Config +0x348.
    /// 0x205eaac38..c3c: eight-byte load at 0x2066a3920, low word bytes 80 4f 12 00, sp+0x390.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaac3c, 0x205eb16a8, 0x205eb1630.
    pub o_348: u32,
    /// Config +0x34c.
    /// Same copy, high word at 0x2066a3924, bytes c0 27 09 00, sp+0x394.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaac3c, 0x205eb1684, 0x205eb1630.
    pub o_34c: u32,
    /// Config +0x350.
    /// 0x205eaac28..c30: w8=0x00037aa0, sp+0x398.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaac30, 0x205eb1660, 0x205eb1630.
    pub o_350: u32,
    /// Config +0x358.
    /// 0x205eaac50..c54: bits 0x4008000000000000 at sp+0x3a0.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaac54, 0x205eb16cc, 0x205eb1630.
    pub o_358: f64,
    /// Config +0x360.
    /// 0x205eaac44..c4c: q0 from 0x2066a3aa0, low lane stored at sp+0x3a8.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaac4c.
    pub o_360: f64,
    /// Config +0x368.
    /// Same q0 store, high lane from 0x2066a3aa8, sp+0x3b0.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Later-write audit: No later write on the specified no-override path; unchanged. All X/S/V-forced stores excluded. No post-copy write.
    /// Potential writers: 0x205eaac4c.
    pub o_368: f64,
    /// Config +0x370; smartBrakeRateControlStrategy.
    /// Zero at sp+0x3b8; 0x205eac7d0..804 can set 2 when the hardware predicate and UseSmartBrake feature allow it; 0x205eac874 writes 0 otherwise; stabilized configure can also reset it to 0 if initialization fails.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Effective value UNKNOWN; initializer uses an inspected builder default or binary zero.
    /// Hardware/feature strategy 0 or 2 and post-copy initialization failure remain UNKNOWN (0x205eac7d0..824, 0x2061fb80c); fallback 0.
    pub o_370: u32,
    /// Config +0x378..0x418: the whole 20-f64 congestion-level threshold table, as
    /// `_VCRateControlCongestionLevel_PopulateCongestionLevelThresholds` 0x2060de4d0 writes it
    /// (q-loads from 0x2066a7610..0x2066a76a8, stp at 0x2060de4e8/4f4/508/524/530) and
    /// 0x205ead24c..270 copies it to config+0x378. Indexed by ShouldRampDown at 0x2065c1e10..e14
    /// (`config+0x378 + range*8`). Entries 5..=9 are also o_3a0..o_3c0 below.
    pub o_378: [f64; 20],
    /// Config +0x3a0.
    /// Output+0x28, high lane of q0 from 0x2066a7640 at 0x2060de4f0; store at output+0x20 by 0x2060de4f4; source constant 0x2066a7648; config copy 0x205ead25c..260.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    pub o_3a0: f64,
    /// Config +0x3a8.
    /// Output+0x30, low lane of q1 from 0x2066a7620 at 0x2060de4dc; stored by 0x2060de4f4; config copy 0x205ead25c..260.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    pub o_3a8: f64,
    /// Config +0x3b0.
    /// Output+0x38, high lane of same q1, source 0x2066a7628; config copy 0x205ead25c..260.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    pub o_3b0: f64,
    /// Config +0x3b8.
    /// Output+0x40, q0 from 0x2066a7650 at 0x2060de4fc; stored by 0x2060de508; config copy 0x205ead264..268.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    pub o_3b8: f64,
    /// Config +0x3c0.
    /// Output+0x48, high lane of same q0, source 0x2066a7658; config copy 0x205ead264..268.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    pub o_3c0: f64,
    /// Config +0x428.
    /// Zero at 0x205eaa560..568, then 0x205eaac68..c6c copies controller+0x7b8 to sp+0x470.
    /// Builder zero/copy coordinates: 0x205eaa560..568; 0x2061fb4e8..500.
    /// Effective value UNKNOWN; initializer uses an inspected builder default or binary zero.
    /// Runtime controller+0x7b8 byte remains UNKNOWN (0x205eaac68..c6c); fallback 0.
    pub o_428: u8,
}

/// Wi-Fi camera data profile with a 4084000 bps cap and cold initial tier.
///
/// Six UNKNOWN runtime inputs use binary defaults, as documented per field.
/// The static profile is audited; runtime inputs and live behavior are not established.
pub fn mode1_camera_defaults() -> VcrcConfig {
    VcrcConfig {
        o_0: 1,
        o_8: &TIER_BITRATES,
        o_10: 17,
        o_14: 31,
        o_18: 0,
        o_1c: 7,
        o_20: 18,
        o_28: 26,
        o_2c: 0,
        o_30: 3,
        o_3c: 1,
        o_40: 1,
        o_44: 1,
        o_48: 1,
        o_4c: 0,
        o_50: 13,
        o_54: 0,
        o_58: 0,
        o_68: 0.15,
        o_60: 0.1,
        o_70: 0.06,
        o_78: 0.1,
        o_1a0: 0.0,
        o_80: 2.0,
        o_88: 2.0,
        o_90: 0.25,
        o_98: 2.0,
        o_a0: 2.0,
        o_a8: 2.0,
        o_b0: 0.001,
        o_c0: 2.0,
        o_c8: 30.0,
        o_d8: 1,
        o_dc: 4,
        o_e0: 3,
        o_e8: 0.9,
        o_f0: 2.0,
        o_f8: -0.05,
        o_108: 0.75,
        o_110: 1.0,
        o_118: 2.0,
        o_120: 0.5,
        o_128: 0.25,
        o_130: 0.01,
        o_138: 0.2,
        o_140: 50,
        o_144: 4,
        o_148: 32000,
        o_14c: 32000,
        o_150: 4,
        o_158: 0.5,
        o_160: 0.75,
        o_168: 1.5,
        o_170: 5.0,
        o_178: 5.0,
        o_182: 1,
        o_183: 0,
        o_180: 1,
        o_18b: 0,
        o_200: f64::from_bits(0x3fe4cccccccccccd),
        o_1f0: 4.0,
        o_1f8: 8.0,
        o_185: 0,
        o_186: 0,
        o_187: 1,
        o_188: 1,
        o_189: 0,
        o_18a: 0,
        o_18e: 1,
        o_194: 0,
        o_198: 0.005,
        o_1a8: 26,
        o_1b0: 20.0,
        o_1b8: 3,
        o_1c0: 5.0,
        o_1c8: 4.0,
        o_1d0: 3.0,
        o_1d8: 5,
        o_1e0: 0.002,
        o_1e8: 4,
        o_1ec: 3,
        o_208: 1,
        o_238: 0.0,
        o_240: 3.0,
        o_268: 0.0,
        o_270: 0.0,
        o_278: 0.0,
        o_288: 0.0,
        o_290: 0.0,
        o_298: 0.0,
        o_2a0: 10.0,
        o_2d0: 0,
        o_2d4: 0,
        o_2d8: 0.5,
        o_2e0: 0.0,
        o_2e8: 0.15,
        o_2f0: 0.01,
        o_2f8: 0.6,
        o_300: 0.03,
        o_308: 1.0,
        o_310: 0.03,
        o_330: 0.01,
        o_338: 0.03,
        o_340: 0.1,
        o_348: 1200000,
        o_34c: 600000,
        o_350: 228000,
        o_358: 3.0,
        o_360: 0.1,
        o_368: 0.5,
        o_370: 0,
        o_378: [
            -1.0, 4.0, 3.5, 3.0, 2.5,
            f64::from_bits(0xbfecccccc0000000), f64::from_bits(0x400cccccc0000000),
            f64::from_bits(0x4009333328000000), f64::from_bits(0x4005999990000000),
            f64::from_bits(0x4001fffff8000000),
            -1.0, 8.0, 3.0, 1.0, 0.5,
            f64::from_bits(0xbfecccccc0000000), f64::from_bits(0x401cccccc0000000),
            f64::from_bits(0x4005999990000000), f64::from_bits(0x3fecccccc0000000),
            f64::from_bits(0x3fdcccccc0000000),
        ],
        o_3a0: f64::from_bits(0xbfecccccc0000000),
        o_3a8: f64::from_bits(0x400cccccc0000000),
        o_3b0: f64::from_bits(0x4009333328000000),
        o_3b8: f64::from_bits(0x4005999990000000),
        o_3c0: f64::from_bits(0x4001fffff8000000),
        o_428: 0,
    }
}
