#!/usr/bin/env python3
"""Run a tiny Terminal-Bench smoke from native Windows.

Terminal-Bench currently constructs a container `/tmp` path with pathlib.Path.
On Windows that becomes `\\tmp`, which Docker sends to the Linux container as a
missing path. This runner patches that diagnostic-only path to a POSIX path so
we can separate local environment failures from Spark adapter failures.
"""

from __future__ import annotations

import argparse
import json
import logging
import shutil
from collections.abc import Iterable
from pathlib import Path, PurePosixPath

from terminal_bench.agents.agent_name import AgentName
from terminal_bench.harness.harness import Harness
from terminal_bench.terminal.docker_compose_manager import DockerComposeManager
from terminal_bench.terminal.tmux_session import TmuxSession


def patch_windows_container_tmp_path() -> None:
    TmuxSession._GET_ASCIINEMA_TIMESTAMP_SCRIPT_CONTAINER_PATH = PurePosixPath(
        "/tmp/get-asciinema-timestamp.sh"
    )
    DockerComposeManager.CONTAINER_TEST_DIR = PurePosixPath("/tests")
    DockerComposeManager.CONTAINER_SESSION_LOGS_PATH = "/logs"


def normalize_line_endings(paths: Iterable[Path]) -> None:
    for root in paths:
        if not root.exists():
            continue
        files = [root] if root.is_file() else [p for p in root.rglob("*") if p.is_file()]
        for path in files:
            try:
                data = path.read_bytes()
            except OSError:
                continue
            if b"\r\n" not in data:
                continue
            path.write_bytes(data.replace(b"\r\n", b"\n"))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--run-id", default="spark-env-smoke-oracle")
    parser.add_argument("--output-path", default=".spark-profile/terminal-bench")
    parser.add_argument("--dataset", default="terminal-bench-core")
    parser.add_argument("--dataset-version", default="0.1.1")
    parser.add_argument("--dataset-cache-root", default="~/.cache/terminal-bench")
    parser.add_argument("--task-id", action="append")
    parser.add_argument("--n-tasks", type=int, default=1)
    parser.add_argument("--agent", default="oracle")
    parser.add_argument("--agent-import-path")
    parser.add_argument("--agent-kwarg", action="append", default=[])
    parser.add_argument("--model")
    parser.add_argument("--no-rebuild", action="store_true")
    parser.add_argument("--clean-run-dir", action="store_true")
    parser.add_argument("--agent-timeout-sec", type=float, default=120)
    parser.add_argument("--test-timeout-sec", type=float, default=120)
    return parser.parse_args()


def parse_agent_kwargs(values: list[str]) -> dict[str, str]:
    kwargs = {}
    for value in values:
        key, separator, parsed_value = value.partition("=")
        if not separator or not key:
            raise ValueError(f"agent kwarg must be key=value: {value}")
        kwargs[key] = parsed_value
    return kwargs


def write_run_metadata(
    output_path: Path,
    run_id: str,
    model: str | None,
    agent_import_path: str | None,
    task_ids: list[str] | None,
) -> None:
    path = output_path / run_id / "spark-run-metadata.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(
            {
                "run_id": run_id,
                "model": model,
                "agent_import_path": agent_import_path,
                "task_ids": task_ids or [],
            },
            indent=2,
        ),
        encoding="utf-8",
    )


def main() -> None:
    args = parse_args()
    patch_windows_container_tmp_path()

    output_path = Path(args.output_path)
    run_path = output_path / args.run_id
    if args.clean_run_dir and run_path.exists():
        shutil.rmtree(run_path)

    dataset_cache_path = (
        Path(args.dataset_cache_root).expanduser() / args.dataset / args.dataset_version
    )
    normalize_line_endings([dataset_cache_path])
    agent_kwargs = parse_agent_kwargs(args.agent_kwarg)
    if args.agent_import_path and args.model and "spark_model" not in agent_kwargs:
        agent_kwargs["spark_model"] = args.model

    harness = Harness(
        output_path=output_path,
        run_id=args.run_id,
        agent_name=None if args.agent_import_path else AgentName(args.agent),
        agent_import_path=args.agent_import_path,
        agent_kwargs=agent_kwargs,
        dataset_name=args.dataset,
        dataset_version=args.dataset_version,
        model_name=args.model,
        no_rebuild=args.no_rebuild,
        cleanup=False,
        log_level=logging.INFO,
        task_ids=args.task_id,
        n_tasks=args.n_tasks if not args.task_id else None,
        n_concurrent_trials=1,
        n_attempts=1,
        global_agent_timeout_sec=args.agent_timeout_sec,
        global_test_timeout_sec=args.test_timeout_sec,
    )
    results = harness.run()
    write_run_metadata(
        output_path,
        args.run_id,
        agent_kwargs.get("spark_model") or args.model,
        args.agent_import_path,
        args.task_id,
    )
    print(results.model_dump_json(indent=2))


if __name__ == "__main__":
    main()
