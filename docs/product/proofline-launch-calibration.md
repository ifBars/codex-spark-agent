# Proofline Windows launch calibration

This harness collects a fail-closed calibration candidate for the exact reviewed Proofline Windows executable. It never changes production countability. Its aggregate always says `countable=false`; independent review of the native protocol, privacy boundary, exact build, twenty attempts, and thresholds is still required before changing the product gate.

## Evidence boundary

Real mode launches only the supplied `.exe` after its SHA-256 and the fixture manifest SHA-256 match the expected values. Before launching, it verifies that the executable contains both the optional native lifecycle sink and its status-schema marker. A build without that hook produces a machine-readable refusal and no launch.

Each attempt gets a unique absolute `SPARK_PROOFLINE_LIFECYCLE_REPORT_PATH` and an absolute `SPARK_PROOFLINE_PROFILE_ROOT` under an external raw-artifact root. The harness follows only the launched PID and its discovered descendants. It captures each process creation time, executable path, and executable SHA-256, then revalidates that identity immediately before termination. A missing or changed identity is never terminated and censors cleanup as uncertain. It inspects owned WebView2 command lines and censors the attempt unless every observed `--user-data-dir` resolves beneath that profile root. Cold attempts reset a disposable profile before every new process. Warm attempts also use new processes, but retain one disposable profile across the warm band.

The native report is the only timing source. A missing, malformed, unwritable, stale, or timed-out report censors the attempt; the harness does not substitute a PowerShell stopwatch or screenshot timestamp. Tauri page-load remains a diagnostic boundary, not first paint. The external observer requires a visible Proofline window, two consecutive geometrically identical non-blank frames, and the image-derived Proofline mark anchor in both frames. Without that anchor, the observation is explicitly ineligible and is never reconciled as UI readiness. It reports only stable visible chrome and never represents that observation as first paint.

TCP inspection is a sampled diagnostic observation over owned PIDs, not enforcement and not proof that the process performed no network activity outside the sampling windows. Deduplicated loopback/bound sockets remain raw diagnostic context, while any sampled non-loopback remote endpoint censors the attempt. The aggregate always states that network verification was not claimed, so polling alone can never make a calibration candidate eligible.

Raw JSON, PID data, TCP endpoints, and screenshots remain under the external artifact root and are disposable. The optional committed aggregate contains only hashes, cold/warm denominators, nearest-rank median and p95 summaries, censored-reason counts, threshold outcomes, and boolean observer results. Its schemas reject additional fields.

Every requested cold or warm ordinal contributes exactly one row. Profile reset, observer startup, application startup, report I/O, and raw-artifact failures become privacy-safe censored rows; they do not abort or shrink the aggregate denominator.

## Run the focused deterministic checks

From the repository root in Windows PowerShell:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\tests\proofline_launch_calibration.Tests.ps1
```

The clearly separate synthetic mode checks helper wiring only. It emits no build SHA or timing-shaped evidence and cannot validate as a real aggregate:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\proofline_launch_calibration.ps1 -SyntheticTest
```

## Run an exact-build calibration

Build Proofline through its existing Bun/Tauri workflow, compute the two expected hashes, then invoke the harness with absolute paths. The fixture manifest is the native bundle manifest at `desktop/proofline/src-tauri/fixtures/wave1-manifest.json`.

```powershell
$prooflineExe = (Resolve-Path '.\desktop\proofline\src-tauri\target\release\proofline.exe').Path
$fixtureManifest = (Resolve-Path '.\desktop\proofline\src-tauri\fixtures\wave1-manifest.json').Path
$buildSha = (Get-FileHash -LiteralPath $prooflineExe -Algorithm SHA256).Hash
$fixtureSha = (Get-FileHash -LiteralPath $fixtureManifest -Algorithm SHA256).Hash
$externalRaw = Join-Path $env:TEMP (Join-Path 'proofline-launch-calibration' ([Guid]::NewGuid().ToString('N')))

powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\proofline_launch_calibration.ps1 `
  -ExecutablePath $prooflineExe `
  -ExpectedExecutableSha256 $buildSha `
  -FixtureManifestPath $fixtureManifest `
  -ExpectedFixtureSha256 $fixtureSha `
  -ColdAttempts 10 `
  -WarmAttempts 10 `
  -RawArtifactRoot $externalRaw `
  -AggregateOutputPath (Join-Path $PWD 'proofline-launch-aggregate.json')
```

Omitting `-RawArtifactRoot` creates a unique root below the operating-system temporary directory. Do not commit that raw directory. Review it locally, retain only the privacy-projected aggregate if authorized, and delete the raw directory after review.

The startup thresholds are applied to host-authoritative `process_to_ui_ready_ms`: cold median at most 3000 ms, cold p95 at most 5000 ms, and warm median at most 1500 ms. Cold and warm samples are never pooled. All attempts remain in the overall and per-band denominators, while censored attempts are excluded from duration summaries and retained through their denominators and reason counts.
