use std::process::Command;

fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(["-C", "../.."])
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() {
    tauri_build::build();
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../fixtures/ownership-map.md");
    println!("cargo:rerun-if-changed=fixtures/wave1-manifest.json");

    let sha = git_output(&["rev-parse", "--short=12", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let dirty = git_output(&["status", "--porcelain", "--untracked-files=normal"])
        .map(|status| !status.is_empty())
        .unwrap_or(true);
    println!("cargo:rustc-env=PROOFLINE_BUILD_GIT_SHA={sha}");
    println!("cargo:rustc-env=PROOFLINE_BUILD_GIT_DIRTY={dirty}");
}
