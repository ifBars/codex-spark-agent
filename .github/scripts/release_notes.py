#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Asset:
    name: str
    label: str
    checksum: str


TARGET_LABELS = {
    "linux-x64": "Linux x64",
    "windows-x64": "Windows x64",
    "macos-arm64": "macOS Apple Silicon",
    "x86_64-unknown-linux-gnu": "Linux x64",
    "x86_64-pc-windows-msvc": "Windows x64",
    "aarch64-apple-darwin": "macOS Apple Silicon",
}


def run_git(args: list[str]) -> str:
    return subprocess.check_output(["git", *args], text=True, encoding="utf-8").strip()


def previous_tag(current_tag: str) -> str | None:
    try:
        tags = run_git(["tag", "--list", "v[0-9]*", "--sort=-v:refname"]).splitlines()
    except subprocess.CalledProcessError:
        return None
    for tag in tags:
        if tag != current_tag:
            return tag
    return None


def commit_subjects(current_tag: str, fallback_commit: str) -> tuple[str | None, list[str]]:
    prior = previous_tag(current_tag)
    if prior:
        range_spec = f"{prior}..HEAD"
    else:
        range_spec = fallback_commit or "HEAD"

    try:
        lines = run_git(["log", "--pretty=format:%h%x09%s", range_spec]).splitlines()
    except subprocess.CalledProcessError:
        lines = []
    return prior, lines[:20]


def read_checksum(path: Path) -> str:
    text = path.read_text(encoding="ascii").strip()
    return text.split()[0] if text else ""


def asset_label(name: str) -> str:
    for target, label in TARGET_LABELS.items():
        if target in name:
            return label
    return "Release asset"


def collect_assets(asset_dir: Path) -> list[Asset]:
    assets: list[Asset] = []
    for archive in sorted(asset_dir.glob("*.zip")):
        checksum_file = archive.with_suffix(archive.suffix + ".sha256")
        checksum = read_checksum(checksum_file) if checksum_file.exists() else ""
        assets.append(Asset(archive.name, asset_label(archive.name), checksum))
    return assets


def release_url(repo: str, tag: str) -> str:
    return f"https://github.com/{repo}/releases/tag/{tag}"


def workflow_url(repo: str, run_id: str) -> str:
    return f"https://github.com/{repo}/actions/runs/{run_id}"


def render_notes(
    *,
    version: str,
    tag: str,
    repo: str,
    commit: str,
    run_id: str,
    assets: list[Asset],
    prior_tag: str | None,
    commits: list[str],
) -> str:
    short_commit = commit[:12] if commit else "unknown"
    compare_link = (
        f"https://github.com/{repo}/compare/{prior_tag}...{tag}" if prior_tag else None
    )

    lines = [
        f"# codex-spark-agent {tag}",
        "",
        f"This release publishes `spark` {version} binaries built from commit `{short_commit}`.",
        "",
        "## Highlights",
        "",
    ]

    if prior_tag:
        lines.append(f"- Includes changes since `{prior_tag}`.")
        if compare_link:
            lines.append(f"- Full comparison: [{prior_tag}...{tag}]({compare_link}).")
    else:
        lines.append("- Initial automated GitHub release for the current crate version.")
        lines.append("- Establishes version-bump driven release publishing for future updates.")

    lines.extend(
        [
            "",
            "## Downloads",
            "",
            "| Platform | Asset | SHA-256 |",
            "| --- | --- | --- |",
        ]
    )

    for asset in assets:
        checksum = f"`{asset.checksum}`" if asset.checksum else "_not recorded_"
        lines.append(f"| {asset.label} | `{asset.name}` | {checksum} |")

    lines.extend(
        [
            "",
            "## Install",
            "",
            "1. Download the archive for your platform.",
            "2. Extract the archive.",
            "3. Put the `spark` executable on your `PATH`, or run it directly from the extracted folder.",
            "",
            "Windows users may need to unblock the downloaded archive before extraction if SmartScreen marks it as internet-downloaded.",
            "",
            "## Validation",
            "",
            "- `cargo test` completed on every release platform before packaging.",
            "- `cargo build --release --bin spark --target <platform>` produced each uploaded binary.",
            f"- Workflow run: [GitHub Actions]({workflow_url(repo, run_id)}).",
            "",
            "## Changes",
            "",
        ]
    )

    if commits:
        for line in commits:
            escaped = re.sub(r"\s+", " ", line).strip()
            lines.append(f"- {escaped}")
    else:
        lines.append("- No commit summary was available when these notes were generated.")

    lines.extend(["", f"Release page: {release_url(repo, tag)}", ""])
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate GitHub release notes.")
    parser.add_argument("--version", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--commit", default="")
    parser.add_argument("--run-id", default="")
    parser.add_argument("--asset-dir", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()

    prior_tag, commits = commit_subjects(args.tag, args.commit)
    assets = collect_assets(args.asset_dir)
    notes = render_notes(
        version=args.version,
        tag=args.tag,
        repo=args.repo,
        commit=args.commit,
        run_id=args.run_id,
        assets=assets,
        prior_tag=prior_tag,
        commits=commits,
    )
    args.output.write_text(notes, encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
