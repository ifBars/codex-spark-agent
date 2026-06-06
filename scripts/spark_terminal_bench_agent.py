from __future__ import annotations

import os
import subprocess
import tempfile
import uuid
from pathlib import Path

from terminal_bench.agents.base_agent import AgentResult, BaseAgent
from terminal_bench.agents.failure_mode import FailureMode
from terminal_bench.terminal.tmux_session import TmuxSession


class SparkTerminalBenchAgent(BaseAgent):
    def __init__(
        self,
        spark_cmd: str = "target/debug/spark.exe",
        spark_model: str | None = None,
        max_turns: int = 12,
        timeout_sec: int = 900,
        **kwargs,
    ):
        super().__init__(**kwargs)
        self.spark_cmd = spark_cmd
        self.spark_model = spark_model
        self.max_turns = str(max_turns)
        self.timeout_sec = timeout_sec

    @staticmethod
    def name() -> str:
        return "spark-terminal-bench"

    def perform_task(
        self,
        instruction: str,
        session: TmuxSession,
        logging_dir: Path | None = None,
    ) -> AgentResult:
        logging_dir = logging_dir or Path(tempfile.mkdtemp(prefix="spark-tbench-"))
        logging_dir.mkdir(parents=True, exist_ok=True)

        prompt_path = logging_dir / "spark-prompt.txt"
        stdout_path = logging_dir / "spark-stdout.txt"
        stderr_path = logging_dir / "spark-stderr.txt"
        scratch_cwd = logging_dir / "spark-host-cwd"
        scratch_cwd.mkdir(exist_ok=True)

        prompt_path.write_text(
            "\n".join(
                [
                    "You are running a Terminal-Bench task.",
                    "Use cmd.exec for all inspection, edits, and verification.",
                    "Do not use fs.read, fs.write, fs.edit, fs.replace, fs.rename, or fs.search; those host filesystem tools do not see the benchmark container.",
                    "cmd.exec is routed into the Linux benchmark container at /app.",
                    "Complete the task by changing the benchmark container state; do not stop after telling the user commands they could run.",
                    "Run the relevant verification command if practical, then give a concise final answer.",
                    "",
                    instruction,
                ]
            ),
            encoding="utf-8",
        )

        env = os.environ.copy()
        env["SPARK_CMD_EXEC_DOCKER_CONTAINER"] = session.container.name
        env["SPARK_CMD_EXEC_DOCKER_WORKDIR"] = "/app"

        command = [
            self.spark_cmd,
            "chat",
            "--cwd",
            str(scratch_cwd),
            "--trace",
            "--profile",
            "--new-session",
            "--session",
            f"terminal-bench-{uuid.uuid4()}",
            "--max-turns",
            self.max_turns,
        ]
        if self.spark_model:
            command.extend(["--model", self.spark_model])
        command.extend(["--prompt-file", str(prompt_path)])

        try:
            completed = subprocess.run(
                command,
                cwd=Path.cwd(),
                env=env,
                text=True,
                capture_output=True,
                timeout=self.timeout_sec,
            )
        except subprocess.TimeoutExpired as error:
            stdout_path.write_text(error.stdout or "", encoding="utf-8")
            stderr_path.write_text(error.stderr or "", encoding="utf-8")
            return AgentResult(failure_mode=FailureMode.AGENT_TIMEOUT)

        stdout_path.write_text(completed.stdout, encoding="utf-8")
        stderr_path.write_text(completed.stderr, encoding="utf-8")

        if completed.returncode != 0:
            return AgentResult(failure_mode=FailureMode.UNKNOWN_AGENT_ERROR)

        return AgentResult()
