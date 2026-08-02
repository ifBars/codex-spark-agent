# Proofline native spike

This is a deliberately non-countable Rust/Slint rendering spike. It preserves the selected Proofline hierarchy without reusing the WebView2/Tauri shell:

- task-history rail;
- evidence-first completed-task detail;
- changed-file and validation evidence;
- collapsed "How Spark worked" row;
- anchored, read-only composer controls; and
- a status ribbon that says `Network gate pending`.

The executable accepts no command-line input and uses only the typed `ProoflineSnapshotV1` fixture compiled into the binary. It has no provider, network, credential, filesystem, shell, git, or run-control integration. It is therefore **not a product build, a usability claim, or a countable lifecycle/network result**.

The first Windows design-QA run remains blocked: the software renderer displays
the evidence surface, but the composer and status ribbon clip at 125% display
scaling. See [`design-qa.md`](design-qa.md). Do not use this build for Wave 1 or a
desktop release.

## Run

```powershell
cargo run --manifest-path desktop/proofline-native/Cargo.toml
```

For the bounded local launch helper, use:

```powershell
.\scripts\proofline_native_spike.ps1
```

## Validate

```powershell
cargo fmt --check --manifest-path desktop/proofline-native/Cargo.toml
cargo check --manifest-path desktop/proofline-native/Cargo.toml
cargo test --manifest-path desktop/proofline-native/Cargo.toml
```
