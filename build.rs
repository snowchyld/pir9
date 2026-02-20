use std::process::Command;

fn main() {
    // Capture rustc version at compile time
    let output = Command::new("rustc")
        .arg("--version")
        .output()
        .expect("Failed to run rustc --version");

    let version = String::from_utf8_lossy(&output.stdout);
    // "rustc 1.93.0 (xxx)" → "1.93.0"
    let version = version
        .trim()
        .strip_prefix("rustc ")
        .unwrap_or(version.trim())
        .split_whitespace()
        .next()
        .unwrap_or("unknown");

    println!("cargo:rustc-env=RUSTC_VERSION={}", version);
}
