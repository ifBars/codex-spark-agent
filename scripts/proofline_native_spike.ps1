param(
    [switch]$CheckOnly
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
$manifestPath = Join-Path $repoRoot "desktop\proofline-native\Cargo.toml"

if ($CheckOnly) {
    cargo check --manifest-path $manifestPath
    exit $LASTEXITCODE
}

cargo run --manifest-path $manifestPath
