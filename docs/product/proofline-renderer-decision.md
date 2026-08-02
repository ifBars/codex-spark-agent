# Proofline renderer and network-boundary decision

Date: 2026-08-01

Status: accepted for a bounded native spike; not release approval

## Decision

Proofline will evaluate a native Slint renderer as the primary desktop path.
The existing Tauri/WebView2 implementation remains a useful visual and event-contract
rehearsal, but it is not the production privacy boundary and must not present an
unqualified `Private` state.

If the Slint spike fails the visual, accessibility, keyboard, or lifecycle gates in
this record, the fallback is a WinUI 3 renderer with a Rust-owned core behind a
versioned, ACL-restricted local IPC boundary. The fallback is Windows-only and more
expensive to integrate, but it has the strongest evaluated path to OS-supported UI
isolation through an AppContainer without network capabilities.

This decision keeps the selected Proofline concept: a narrow task-history rail, an
evidence-first completed-task document, collapsed model activity, an anchored
composer, and a persistent status ribbon. It changes the rendering substrate, not
the product hierarchy.

## Threat and authority boundary

Proofline has two distinct network states:

1. Before an explicit model action, the renderer and idle desktop process tree make
   no non-loopback connection.
2. After an explicit model action, only the Rust-owned provider authority may make
   the labeled provider connection needed for that action.

The renderer never owns provider credentials, pricing authority, shell execution,
filesystem or Git mutation, approvals, checkpoints, or direct provider transport.
It receives versioned presentation events and emits a narrow set of typed user
intents. A compromised renderer must not be sufficient to exercise worker-plane
authority.

The footer reports observed state instead of a blanket privacy promise. The native
spike therefore shows `Network gate pending`. A future live build should distinguish
at least `Local workspace`, `Model network idle`, and the active provider destination.

## Why WebView2 does not meet the gate

WebView2 is a multi-process runtime with browser, renderer, GPU, utility, crash, and
other helper processes. Microsoft documents required and optional diagnostic data,
and does not expose a supported global network-off control for the owned process
group. [`WebResourceRequested`](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/webresourcerequested)
intercepts web-resource requests; it is not a process-level socket policy. Browser
flags are explicitly not a durable production contract, and a fixed runtime pins a
runtime version without creating a network boundary.

Microsoft's WebView2 team has also documented the shared-child-process firewall
problem and suggested WDAC AppId plus firewall policy for exact targeting. That
route requires privileged machine policy and can require reboot, so it violates the
consumer-product requirement of no administrator action and no broad device rule.
Windows Filtering Platform can filter by application identity, but adding filters
requires filter-engine write access that an ordinary user process does not receive.

Sources:

- [WebView2 process model](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-model)
- [WebView2 privacy and diagnostic data](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/data-privacy)
- [WebView2 browser flags](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/webview-features-flags)
- [WebView2 child-process firewall discussion](https://github.com/MicrosoftEdge/WebView2Feedback/issues/369)
- [Windows Filtering Platform access control](https://learn.microsoft.com/en-us/windows/win32/fwp/access-control)
- [AppContainer isolation](https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation)

The first lifecycle smoke already observed non-loopback traffic from owned WebView2
processes. That result was not countable because stable visual frames were not
established, but it is enough to reject an unqualified privacy claim and to stop
treating socket polling as enforcement.

## Options considered

| Option | Whole renderer boundary | No admin or device policy | Fit for Proofline | Decision |
| --- | --- | --- | --- | --- |
| CSP, request interception, browser flags | No | Yes | Existing rehearsal only | Reject |
| Fixed WebView2 runtime or isolated profile | No | Yes | Useful reproducibility, not containment | Reject |
| Proxy or loopback broker | No; runtime traffic can bypass it | Usually | Does not prove the claim | Reject |
| Job object | Tracks process lifetime, not sockets | Yes | Useful cleanup only | Reject |
| WFP, firewall, or WDAC rules | Potentially | No | Machine policy is out of scope | Reject |
| WebView2 inside AppContainer | Conditional and unproven for Tauri/Wry | Conditional | High integration risk | Defer |
| Native Slint renderer | Removes browser-runtime traffic class | Yes | Best Rust and visual-spike fit | Primary spike |
| WinUI 3 AppContainer plus Rust core | OS-supported UI isolation path | Conditional packaging POC | Strongest Windows fallback | Fallback |

Selecting Slint does not itself prove that Proofline is private. It removes the
embedded browser and makes renderer authority narrow enough to audit. The network
claim still requires runtime evidence and, where possible, enforceable process
isolation.

## Native spike

The first Slint slice is deliberately fixture-only. It must render the selected
Proofline hierarchy from a typed `ProoflineSnapshotV1` without a provider client,
network crate, credentials, shell, filesystem, Git, or run-control command path.
Slint is being evaluated because it is Rust-native, declarative, supports desktop
rendering without a browser, and exposes an OS accessibility tree. Its desktop
maturity, keyboard behavior, text quality, and royalty-free attribution requirement
remain explicit risks. See [Slint desktop support](https://docs.slint.dev/latest/docs/slint/guide/platforms/desktop/),
[backends and renderers](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers),
and [licensing](https://slint.dev/pricing).

The spike passes only when all of these are true:

- the exact built process tree contains no WebView2 or Chromium process;
- five cold and five warm launches render a named Proofline anchor without a crash;
- raw attempts and censored failures are published, with cold median at or below
  three seconds and warm median at or below 1.5 seconds;
- keyboard-only task selection and the collapsed activity toggle work;
- UI Automation or Narrator exposes named task rail, main evidence region,
  composer controls, and status ribbon;
- an event-based or enforceable verifier observes no idle non-loopback connection;
  socket polling may be retained only as secondary diagnostics; and
- source and dependency review confirms that renderer code has no worker-plane or
  provider-network authority.

Any failed gate keeps the footer at `Network gate pending` and blocks the 10-cold /
10-warm release protocol. This bounded spike is not a participant-countable Wave 1
build and does not justify a desktop release.

## Product implications

- `desktop/proofline/` remains the selected visual reference and browser/Sites
  rehearsal; it is not deleted or silently promoted.
- The Rust harness remains authoritative for events, execution, usage, checkpoints,
  and provider transport.
- Issue #13 remains open until the chosen architecture has enforceable or
  event-based evidence, not merely a toolkit choice.
- Desktop release eligibility still requires the native lifecycle and privacy gates,
  then the participant protocol. The CLI release remains independent.
