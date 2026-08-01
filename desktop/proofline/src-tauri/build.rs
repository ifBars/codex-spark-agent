use std::{fs, process::Command};

const REPO_ROOT: &str = "../../..";

fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(["-C", REPO_ROOT])
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn track_git_path(pathspec: &str) {
    if let Some(path) = git_output(&["rev-parse", "--git-path", pathspec]) {
        println!("cargo:rerun-if-changed={path}");
    }
}

fn main() {
    tauri_build::build();
    println!("cargo:rerun-if-changed=../../../.git");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=../fixtures/ownership-map.md");
    println!("cargo:rerun-if-changed=../src");
    println!("cargo:rerun-if-changed=../assets");
    println!("cargo:rerun-if-changed=../scripts");
    println!("cargo:rerun-if-changed=../worker");
    println!("cargo:rerun-if-changed=../index.html");
    println!("cargo:rerun-if-changed=../package.json");
    println!("cargo:rerun-if-changed=../bun.lock");
    println!("cargo:rerun-if-changed=../vite.config.mjs");
    println!("cargo:rerun-if-changed=fixtures/wave1-manifest.json");
    track_git_path("HEAD");
    track_git_path("index");
    if let Some(head_path) = git_output(&["rev-parse", "--git-path", "HEAD"]) {
        if let Ok(head) = fs::read_to_string(head_path) {
            if let Some(reference) = head.trim().strip_prefix("ref: ") {
                track_git_path(reference);
            }
        }
    }

    let sha = git_output(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let dirty = git_output(&["status", "--porcelain", "--untracked-files=normal"])
        .map(|status| !status.is_empty())
        .unwrap_or(true);
    println!("cargo:rustc-env=PROOFLINE_BUILD_GIT_SHA={sha}");
    println!("cargo:rustc-env=PROOFLINE_BUILD_GIT_DIRTY={dirty}");
}
