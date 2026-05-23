fn main() {
    let git_sha = std::env::var("PIT2SOP_GIT_SHA")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(git_sha)
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=PIT2SOP_GIT_SHA={git_sha}");
    println!("cargo:rerun-if-env-changed=PIT2SOP_GIT_SHA");
    println!("cargo:rerun-if-changed=../../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../../.git/refs/heads/main");
    tauri_build::build()
}

fn git_sha() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let sha = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if sha.is_empty() {
        return None;
    }
    Some(if git_is_dirty() {
        format!("{sha}-dirty")
    } else {
        sha
    })
}

fn git_is_dirty() -> bool {
    std::process::Command::new("git")
        .args(["diff", "--quiet", "HEAD", "--"])
        .status()
        .map(|status| !status.success())
        .unwrap_or(false)
}
