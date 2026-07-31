"""Harbor installed-agent adapter for benchmarking Spark on Terminal-Bench."""

from __future__ import annotations

import os
import shlex
from pathlib import Path, PurePosixPath

from harbor.agents.installed.base import BaseInstalledAgent, with_prompt_template
from harbor.environments.base import BaseEnvironment
from harbor.models.agent.context import AgentContext
from harbor.models.trial.paths import EnvironmentPaths


class SparkAgent(BaseInstalledAgent):
    """Install a local Spark build in each Harbor task container."""

    _OUTPUT_FILENAME = "spark.txt"

    def __init__(
        self,
        *args,
        spark_binary_path: str | None = None,
        spark_auth_path: str | None = None,
        reasoning_effort: str = "medium",
        **kwargs,
    ) -> None:
        super().__init__(*args, **kwargs)
        self._spark_binary_path = Path(
            spark_binary_path
            or os.environ.get("SPARK_TB_BINARY", "target/release/spark")
        ).resolve()
        self._spark_auth_path = Path(
            spark_auth_path
            or os.environ.get(
                "SPARK_TB_AUTH",
                Path.home() / ".spark-codex" / "auth.json",
            )
        ).resolve()
        if reasoning_effort not in {"low", "medium", "high", "xhigh"}:
            raise ValueError(
                "reasoning_effort must be one of low, medium, high, or xhigh"
            )
        self._reasoning_effort = reasoning_effort

    @staticmethod
    def name() -> str:
        return "spark"

    def get_version_command(self) -> str | None:
        return "spark --help | head -n 1"

    async def install(self, environment: BaseEnvironment) -> None:
        if not self._spark_binary_path.is_file():
            raise FileNotFoundError(
                f"Spark Linux binary not found: {self._spark_binary_path}"
            )
        if not self._spark_auth_path.is_file():
            raise FileNotFoundError(f"Spark auth file not found: {self._spark_auth_path}")

        await environment.upload_file(
            self._spark_binary_path, "/installed-agent/spark"
        )
        await environment.upload_file(
            self._spark_auth_path, "/installed-agent/auth.json"
        )
        await self.exec_as_root(
            environment,
            command=(
                "if command -v apt-get >/dev/null 2>&1; then "
                "  apt-get update && "
                "  DEBIAN_FRONTEND=noninteractive apt-get install -y "
                "    ca-certificates git ripgrep; "
                "elif command -v apk >/dev/null 2>&1; then "
                "  apk add --no-cache ca-certificates git ripgrep; "
                "else "
                "  echo 'Unsupported package manager for Spark dependencies' >&2; "
                "  exit 1; "
                "fi && "
                "update-ca-certificates >/dev/null 2>&1 || true; "
                "chmod 0755 /installed-agent/spark && "
                "chmod 0600 /installed-agent/auth.json && "
                "ln -sf /installed-agent/spark /usr/local/bin/spark"
            ),
        )
        await self.exec_as_agent(
            environment,
            command=(
                'mkdir -p "$HOME/.spark-codex" && '
                'cp /installed-agent/auth.json "$HOME/.spark-codex/auth.json" && '
                'chmod 0600 "$HOME/.spark-codex/auth.json"'
            ),
        )

    @with_prompt_template
    async def run(
        self,
        instruction: str,
        environment: BaseEnvironment,
        context: AgentContext,
    ) -> None:
        del context
        model = self.model_name or "gpt-5.3-codex-spark"
        output_path = PurePosixPath(EnvironmentPaths.agent_dir) / self._OUTPUT_FILENAME
        command = (
            "spark chat "
            "--cwd . "
            "--new-session "
            "--profile "
            f"--model {shlex.quote(model.split('/')[-1])} "
            f"--reasoning-effort {shlex.quote(self._reasoning_effort)} "
            f"-- {shlex.quote(instruction)} "
            f"2>&1 </dev/null | tee {shlex.quote(output_path.as_posix())}"
        )
        await self.exec_as_agent(
            environment,
            command=command,
            env={"NO_COLOR": "1"},
        )

    def populate_context_post_run(self, context: AgentContext) -> None:
        # Spark's profile summary remains in spark.txt. Harbor's verifier reward is
        # the benchmark score; token/cost fields are intentionally left unset.
        del context
