use std::process::Command;

fn main() {
    let describe = Command::new("git")
        .args(["describe", "--always", "--dirty", "--abbrev=12"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    println!("cargo:rustc-env=SONA_GIT_DESCRIBE={describe}");
    // The crates directory too, so an edit flips `-dirty` without a commit.
    for path in ["../../.git/HEAD", "../../.git/index", "../../.git/refs", "../../crates"] {
        println!("cargo:rerun-if-changed={path}");
    }
}
