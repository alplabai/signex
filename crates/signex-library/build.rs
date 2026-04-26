//! Build script for `signex-library`.
//!
//! libgit2-sys 0.17 (transitively pulled by `git2 = "0.19"` under the
//! `local-git` feature) calls `OpenProcessToken`, `CryptGenRandom`,
//! `RegOpenKeyExW`, etc. but neglects to link `advapi32` itself, so the
//! Windows MSVC linker fails when building any binary target (tests,
//! examples, dependent bins). Emit the link hint here so the feature
//! works out of the box on Windows.

fn main() {
    if std::env::var_os("CARGO_FEATURE_LOCAL_GIT").is_none() {
        return;
    }
    let target = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target == "windows" {
        println!("cargo:rustc-link-lib=advapi32");
    }
}
