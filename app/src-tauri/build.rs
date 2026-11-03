fn main() {
    tauri_build::build();
    // The integration tests link the real WebView2 loader and need the Common Controls v6
    // manifest, or they fail to start (STATUS_ENTRYPOINT_NOT_FOUND). Tests only: the app's own
    // binary gets its manifest from tauri-build.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        println!("cargo:rerun-if-changed=test.manifest");
        println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-tests=/MANIFESTINPUT:{dir}/test.manifest");
    }
}
