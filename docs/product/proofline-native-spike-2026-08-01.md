# Proofline native renderer spike

Date: 2026-08-01

Status: visual gate failed; no desktop release

## Outcome

The first Rust/Slint Proofline spike proved that the selected evidence surface can
run without WebView2, but it did not pass the desktop gate. The native window first
opened as a blank surface on the default FemtoVG path. Selecting Slint's software
renderer made the task rail, evidence document, changed files, validation, and
collapsed activity row visible. At 125% Windows display scaling, however, the
anchored composer and persistent status ribbon remained clipped outside the usable
client area.

That is a P1 product failure because model, reasoning, workspace authority, pricing
availability, and `Network gate pending` are required persistent states. The build
is a useful substrate experiment, not a preview release or participant-countable
Wave 1 candidate. Full visual findings are in
[`desktop/proofline-native/design-qa.md`](../../desktop/proofline-native/design-qa.md).

## What was established

- `ProoflineSnapshotV1` is a typed, fixture-only input with deterministic
  presentation mapping.
- The renderer has no shell, filesystem, Git, credential, provider, pricing, or
  run-control integration.
- Normal dependency inspection found no `reqwest`, `ureq`, `hyper`, `tokio`, Wry,
  WebView, WebKit, or Chromium dependency.
- The final bounded diagnostic observed one `proofline-native` process and no
  child process in both attempts. No WebView2 or Chromium process was present.
- One cold and one warm diagnostic observed a window handle in 61 ms and 34 ms,
  respectively.
- Socket polling observed zero owned non-loopback TCP connections in both attempts.
- Two deterministic fixture and presentation tests passed.

The exact diagnostic binary SHA-256 was
`9C5012F11D48CC48687A02143711BB23E96071770FAA95FCD0AE8E6C6914D429`.
Both launched process identities were rechecked against that binary before cleanup.

These are not countable lifecycle or privacy results. A window handle is not a
stable first-visible Proofline anchor, and socket polling cannot prove that a short
connection did not occur. No event-based network observer or OS-enforced network
boundary was present.

## Failed and unrun gates

- Design QA: failed because composer and status ribbon clip at 125% scaling.
- DPI matrix: 100%, 150%, and 200% were not run after the 125% failure.
- Accessibility: UI Automation, Narrator, and complete keyboard traversal were not
  validated.
- Network: an event-based or enforceable idle-network verifier is still absent.
- Lifecycle: five cold and five warm stable-anchor launches were not eligible.
- Wave 1: no participant is recruited on this build.

## PM decision

Keep the Tauri/browser implementation as the visual and Sites rehearsal and keep
this Slint crate as the native falsification spike. Do not extend either into live
provider control.

The one bounded Slint DPI-layout correction has now run and failed at the host's
125% display scale. It removed the post-show physical-size workaround, introduced
a layout-driven root, and added compiler-accepted keyboard/accessibility semantics,
but the composer and status ribbon remained clipped. Per the recorded stop
condition, Slint product work is closed and the next renderer spike is WinUI 3 plus
the Rust-core boundary described in
[`proofline-renderer-decision.md`](proofline-renderer-decision.md).

The required WinUI bootstrap was attempted with the repository-selected workflow,
but the WinGet configuration was canceled before applying its units. A read-only
audit found Windows build 19045, Developer Mode enabled, .NET SDKs 6/8/9, and
Windows SDKs through 26100, but no installed `winui` template and no Visual Studio
instance with the Windows App SDK C# component. The fallback implementation must
not be hand-authored around that missing template; resume from the official
scaffold after the toolchain installation succeeds.

The 10-cold / 10-warm protocol and a desktop version tag remain blocked. The current
published CLI release is unaffected.
