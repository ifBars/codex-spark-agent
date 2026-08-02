# Proofline native lifecycle and privacy gate

`scripts/proofline_native_lifecycle_gate.ps1` is the reusable native Proofline
diagnostic protocol. Its defaults are five cold plus five warm attempts; the same
script accepts `-ColdAttempts 10 -WarmAttempts 10` when the release protocol is
otherwise eligible.

Here, *cold* means a fresh process launch; it does not claim an impossible
unprivileged purge of Windows file caches. *Warm* means a subsequent launch of the
same binary in the same diagnostic session, so retained operating-system cache is
possible. Both definitions are emitted in the aggregate artifact.

Every attempt is retained in the denominator. The runner binds the run to the
caller-provided SHA-256 of the executable, captures identities for the root and
observed descendants, records first window-handle and named UI Automation anchor
timing, samples owned TCP connections, and uses creation-time plus binary identity
before cleanup.

The timing fields have deliberate limits: a window handle is not a visible frame,
and a UI Automation anchor is an accessibility-tree observation rather than a
stable pixel proof. The raw JSON records those limits and never labels either as a
visual assertion.

The current Windows connection method is `sampled_windows_tcp_table`, using
`Get-NetTCPConnection` for observed owned PIDs. It is a best-available secondary
diagnostic, not an event trace, enforcement mechanism, or proof that no short-lived
connection occurred. Therefore every aggregate stays `privacy_gate: pending` and
`countable: false`; even a 10+10 run cannot become release-eligible until an
event-based or enforceable network boundary is separately established.

This automation therefore cannot satisfy [issue #13](https://github.com/ifBars/codex-spark-agent/issues/13) by itself. It is retained to make the remaining gap measurable without silently promoting sampled TCP output into a privacy guarantee.

## Validate the protocol

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\tests\proofline_native_lifecycle_gate.Tests.ps1
```

## Run a bounded 5+5 diagnostic

```powershell
$exe = Resolve-Path .\desktop\proofline-native\target\debug\proofline-native.exe
$sha = (Get-FileHash $exe -Algorithm SHA256).Hash
$raw = Join-Path $env:TEMP ('proofline-native-lifecycle-' + [guid]::NewGuid().ToString('N'))

powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\proofline_native_lifecycle_gate.ps1 `
  -ExecutablePath $exe `
  -ExpectedExecutableSha256 $sha `
  -RawArtifactRoot $raw
```

Raw evidence contains PIDs and endpoint observations, so retain it only for the
approved diagnostic window and publish a redacted summary rather than raw data.
