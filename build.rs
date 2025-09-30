// build.rs
use chrono::Utc;
use std::process::Command;

fn main() {
    // Git commit (short)
    let git_sha = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    // rustc version info
    let rustc_v = Command::new("rustc")
        .arg("-Vv")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.replace('\n', " | "))
        .unwrap_or_else(|| "rustc -Vv unavailable".to_string());

    let utc = Utc::now();
    println!("cargo:rustc-env=BUILD_GIT_SHA={}", git_sha);
    println!("cargo:rustc-env=BUILD_RUSTC={}", rustc_v);
    println!("cargo:rustc-env=BUILD_TS_UNIX={}", utc.to_rfc3339());

    // Re-run if HEAD changes
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");
}
