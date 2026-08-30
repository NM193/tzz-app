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

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("ocr");

    let status = Command::new("swiftc")
        .args(["-O", "-o"])
        .arg(&out)
        .arg("helpers/ocr.swift")
        .args(["-framework", "Vision", "-framework", "AppKit"])
        .status();

    match status {
        Ok(status) if status.success() => {}
        Ok(status) => panic!("swiftc failed to build the OCR helper ({status})"),
        Err(error) => panic!(
            "swiftc is needed to build the OCR helper but could not be run ({error}). \
             Install the command line tools with: xcode-select --install"
        ),
    }
}
