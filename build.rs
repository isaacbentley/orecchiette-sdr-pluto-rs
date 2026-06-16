fn main() {
    // libiio on macOS ships as `iio.framework` (Homebrew installs it under
    // /opt/homebrew/Frameworks on Apple Silicon, /usr/local/Frameworks on
    // Intel). The `industrial-io` link records the install name
    // `@rpath/iio.framework/...` but nothing emits an LC_RPATH, so every
    // binary and test executable aborts at dyld load with
    // "Library not loaded: @rpath/iio.framework". Emit an rpath for
    // whichever standard framework directory actually has the framework;
    // on hosts without it (Linux, CI containers) this is a no-op.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        for dir in [
            "/opt/homebrew/Frameworks",
            "/usr/local/Frameworks",
            "/Library/Frameworks",
        ] {
            if std::path::Path::new(dir).join("iio.framework").exists() {
                println!("cargo::rustc-link-arg=-Wl,-rpath,{dir}");
                break;
            }
        }
    }
}
