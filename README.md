# Codex Spark Agent

A fast, experimental coding agent built around `gpt-5.3-codex-spark`.

<p align="center">
  <img src="docs/images/spark-desktop-light.png" width="49%" alt="Spark Desktop new chat screen in light mode">
  <img src="docs/images/spark-desktop-dark.png" width="49%" alt="Spark Desktop new chat screen in dark mode">
</p>

Spark combines a focused Rust agent loop with native coding tools, persistent chat sessions, skills, compaction, traces, and repeatable benchmarks. It is independent research software, not an official OpenAI project.

[Download Spark](https://github.com/ifBars/codex-spark-agent/releases/latest) · [Spark Desktop](https://github.com/ifBars/t3code/releases/latest) · [SparkBench](https://ifbars.github.io/codex-spark-agent/) · [Development docs](docs/development.md)

## Quick start

Download the latest release for Windows, Linux, or macOS, extract it, and put `spark` on your `PATH`.

```text
spark setup
spark chat
```

Spark uses ChatGPT/Codex OAuth. You can also run a single task directly:

```text
spark chat "Review this repository and suggest the highest-impact fix."
```

## What is included

- Interactive and one-shot coding sessions
- Native filesystem, shell, Git, web, and MCP tools
- Repo skills and multi-agent workflows
- Trace inspection, usage history, and context compaction
- SparkBench for measured quality, cost, and latency comparisons

Run `spark --help` for the complete CLI. See the [development guide](docs/development.md) for source builds, tests, configuration, and architecture.

## Safety

Spark can edit files and run commands with your account's permissions. Use a clean worktree or disposable environment, and treat saved traces as sensitive.

## License

[MIT](LICENSE)
