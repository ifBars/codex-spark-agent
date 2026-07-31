# Terminal-Bench 2.0

This adapter runs the checked-out Spark harness as an installed Harbor agent,
inside the unmodified Terminal-Bench task containers.

## Prerequisites

- Docker Desktop with the Linux engine running
- Harbor installed (`uv tool install harbor`)
- A Linux Spark binary built from the commit under test
- A valid Spark auth file

From PowerShell, build the current checkout in a bullseye container with OpenSSL
statically linked. Terminal-Bench 2.0 includes both bullseye and newer glibc
images, and many of its minimal images do not preinstall OpenSSL:

```powershell
docker run --rm `
  --volume "${PWD}:/work" `
  --workdir /work `
  rust:1.90-bullseye `
  bash -c 'apt-get update &&
    apt-get install -y pkg-config libssl-dev &&
    OPENSSL_STATIC=1 cargo build --release --bin spark'

$env:SPARK_TB_BINARY = (Resolve-Path .\target\release\spark).Path
$env:SPARK_TB_AUTH = (Resolve-Path "$env:USERPROFILE\.spark-codex\auth.json").Path

harbor run `
  --dataset terminal-bench@2.0 `
  --agent-import-path scripts.terminal_bench.spark_harbor_agent:SparkAgent `
  --model gpt-5.3-codex-spark `
  --agent-kwarg version=0.4.1 `
  --n-tasks 1 `
  --n-concurrent 1
```

Remove `--n-tasks 1` for the full dataset. Harbor writes its original verifier
rewards and trial logs under the selected jobs directory. Keep the default
environment deletion behavior because each container receives a copy of the
local auth file.
