use std::io::Result;

fn main() -> Result<()> {
    // Flutter profile builds use the Rust release profile. Refuse the old CI
    // placeholders before they can ship in another apparently successful APK.
    if std::env::var("PROFILE").unwrap_or_default() == "release" {
        let legacy_cert = std::fs::read("certs/legacy-fairplay/fairplay.crt")?;
        let legacy_key = std::fs::read("certs/legacy-fairplay/fairplay.pem")?;
        for serial in (193..=201).chain(std::iter::once(208)) {
            for (extension, legacy) in [("crt", &legacy_cert), ("pem", &legacy_key)] {
                let path = format!("certs/fairplay/4056631661436364584235346952{serial}.{extension}");
                println!("cargo:rerun-if-changed={path}");
                let bytes = std::fs::read(&path)?;
                if bytes.is_empty() || bytes == *legacy {
                    return Err(std::io::Error::other(format!(
                        "Invalid FairPlay signing material in {path}; run scripts/copy_fairplay_certs.sh from the app checkout"
                    )));
                }
            }
        }
    }
    if std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default() != "wasm32" {
        let root = std::path::Path::new("third_party/evs-reference");
        let mut build = cc::Build::new();
        // EVS and FDK-AAC both expose these reference math tables. Namespace
        // EVS at compile time so both codecs can coexist in the Android library.
        for symbol in ["exp2_tab_long", "exp2w_tab_long", "exp2x_tab_long", "t_qua_gain7b"] {
            build.define(symbol, Some(format!("obim_evs_{symbol}").as_str()));
        }
        build.include(root.join("lib_com")).include(root.join("lib_dec"))
            .include(root.join("lib_enc"))
            .warnings(false).flag_if_supported("-std=gnu99")
            .file(root.join("obim_decoder.c"));
        for directory in ["lib_com", "lib_dec"] {
            for file in std::fs::read_dir(root.join(directory))? {
                let path = file?.path();
                if path.extension().is_some_and(|extension| extension == "c") {
                    build.file(path);
                }
            }
        }
        println!("cargo:rerun-if-changed={}", root.display());
        build.compile("obim_evs_decoder");
        println!("cargo:rustc-link-lib=m");
    }
    let mut prost_build = prost_build::Config::new();
    // Enable a protoc experimental feature.
    prost_build.protoc_arg("--experimental_allow_proto3_optional");
    prost_build.compile_protos(
        &[
            "src/icloud/mmcs.proto",
            "src/ids/ids.proto",
            "src/ids/qr.proto",
            "src/facetime.proto",
            "src/avconference.proto",
            "src/statuskit.proto",
            "src/imessage/cloud_messages.proto",
            "src/passwords.proto",
        ],
        &["src/"],
    )?;
    Ok(())
}
