use std::path::PathBuf;
use std::process::Command;

fn main() {
    build_ocr_helper();
    tauri_build::build()
}

/// Compile the Vision OCR helper and hand its path to the crate.
///
/// It is embedded in the binary rather than bundled as a resource: the resource
/// directory differs between `tauri dev` and a packaged app, and a helper that
/// cannot be found is a feature that silently does nothing.
fn build_ocr_helper() {
    println!("cargo:rerun-if-changed=helpers/ocr.swift");
    // `tauri build` sets this from bundle.macOS.minimumSystemVersion, and the
    // Rust is built for it. swiftc does not read it: left alone, the helper
    // asks for the macOS of the Mac that built it, and screen text fails on
    // anything older. Unset -- `tauri dev`, `cargo test` -- nothing changes.
    println!("cargo:rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("ocr");

    let mut swiftc = Command::new("swiftc");
    swiftc
        .args(["-O", "-o"])
        .arg(&out)
        .arg("helpers/ocr.swift")
        .args(["-framework", "Vision", "-framework", "AppKit"]);
    if let Ok(minimum) = std::env::var("MACOSX_DEPLOYMENT_TARGET") {
        let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
        let arch = if arch == "aarch64" { "arm64" } else { arch.as_str() };
        swiftc.arg("-target").arg(format!("{arch}-apple-macos{minimum}"));
    }
    let status = swiftc.status();

    match status {
        Ok(status) if status.success() => {}
        Ok(status) => panic!("swiftc failed to build the OCR helper ({status})"),
        Err(error) => panic!(
            "swiftc is needed to build the OCR helper but could not be run ({error}). \
             Install the command line tools with: xcode-select --install"
        ),
    }
}
