#!/usr/bin/env python3
from __future__ import annotations

import argparse
import csv
import json
from collections import defaultdict
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runs-path", default=".spark-profile/terminal-bench")
    parser.add_argument("--output-json", default=".spark-profile/terminal-bench/model-summary.json")
    parser.add_argument("--output-csv", default=".spark-profile/terminal-bench/model-summary.csv")
    parser.add_argument("--output-md", default=".spark-profile/terminal-bench/model-summary.md")
    parser.add_argument("--run-id-prefix")
    parser.add_argument("--require-model-metadata", action="store_true")
    return parser.parse_args()


def read_json(path: Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return {}


def collect_rows(
    runs_path: Path,
    run_id_prefix: str | None = None,
    require_model_metadata: bool = False,
) -> list[dict]:
    rows = []
    for result_path in sorted(runs_path.glob("*/results.json")):
        run_dir = result_path.parent
        if run_id_prefix and not run_dir.name.startswith(run_id_prefix):
            continue
        metadata = read_json(run_dir / "spark-run-metadata.json")
        if require_model_metadata and not metadata.get("model"):
            continue
        results = read_json(result_path)
        model = metadata.get("model") or results.get("model") or "unknown"
        for result in results.get("results", []):
            rows.append(
                {
                    "run_id": run_dir.name,
                    "model": model,
                    "task_id": result.get("task_id"),
                    "resolved": bool(result.get("is_resolved")),
                    "failure_mode": result.get("failure_mode"),
                    "parser_results": result.get("parser_results") or {},
                    "trial_started_at": result.get("trial_started_at"),
                    "trial_ended_at": result.get("trial_ended_at"),
                }
            )
    return rows


def summarize(rows: list[dict]) -> dict:
    by_model = defaultdict(list)
    for row in rows:
        by_model[row["model"]].append(row)

    models = []
    for model, model_rows in sorted(by_model.items()):
        resolved = sum(1 for row in model_rows if row["resolved"])
        total = len(model_rows)
        models.append(
            {
                "model": model,
                "resolved": resolved,
                "total": total,
                "accuracy": resolved / total if total else 0.0,
            }
        )
    return {"models": models, "rows": rows}


def write_csv(path: Path, rows: list[dict]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(
            handle,
            fieldnames=[
                "run_id",
                "model",
                "task_id",
                "resolved",
                "failure_mode",
                "trial_started_at",
                "trial_ended_at",
            ],
        )
        writer.writeheader()
        for row in rows:
            writer.writerow({key: row.get(key) for key in writer.fieldnames})


def write_markdown(path: Path, summary: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    lines = [
        "# Terminal-Bench Model Summary",
        "",
        "## By Model",
        "",
        "| Model | Resolved | Total | Accuracy |",
        "| --- | ---: | ---: | ---: |",
    ]
    for model in summary["models"]:
        lines.append(
            f"| `{model['model']}` | {model['resolved']} | {model['total']} | {model['accuracy']:.1%} |"
        )
    lines.extend(
        [
            "",
            "## Runs",
            "",
            "| Model | Run | Task | Result | Failure Mode |",
            "| --- | --- | --- | --- | --- |",
        ]
    )
    for row in summary["rows"]:
        result = "pass" if row["resolved"] else "fail"
        lines.append(
            f"| `{row['model']}` | `{row['run_id']}` | `{row['task_id']}` | {result} | `{row['failure_mode']}` |"
        )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> None:
    args = parse_args()
    summary = summarize(
        collect_rows(
            Path(args.runs_path),
            run_id_prefix=args.run_id_prefix,
            require_model_metadata=args.require_model_metadata,
        )
    )
    Path(args.output_json).write_text(json.dumps(summary, indent=2), encoding="utf-8")
    write_csv(Path(args.output_csv), summary["rows"])
    write_markdown(Path(args.output_md), summary)
    print(json.dumps(summary["models"], indent=2))


if __name__ == "__main__":
    main()
