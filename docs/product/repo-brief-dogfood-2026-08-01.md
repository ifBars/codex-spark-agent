# Repo Brief dogfood: local-only enforcement audit

Date: 2026-08-01

This is one authenticated product-runtime observation, not a benchmark or a
performance claim. The run asked Spark to locate Repo Brief's local-filesystem
enforcement and identify a remaining bypass path.

## Result

- Output contract: passed all four required sections and 13 file-line citations.
- Safety contract: passed; only `fs.read`, `fs.list`, and `fs.search` executed.
- Runtime: 50,081 ms.
- Model requests: 20.
- Tool calls: 37 (`fs.read` 23, `fs.search` 13, `fs.list` 1).
- Provider usage: 437,993 input tokens, 325,376 cached input tokens, 8,754
  output tokens, and 446,747 total tokens across completed response payloads.
- Compaction: one remote compaction at 160,699 input characters, reduced to
  16,459 characters.

## Human acceptance review

Status: **needs substantive rework**.

The brief correctly located the advertised-tool and invocation-time guards. It
then presented mutable `read_roots` as the remaining bypass path without first
confirming the Repo Brief runner's actual configuration. `AgentRunner` starts
with an empty `read_roots` list, and `build_readonly_runner` does not call
`set_read_roots`; the reported risk is conditional architecture surface, not an
observed Repo Brief bypass.

The run therefore passes the syntactic contract but fails the beta acceptance
gate. This is why required headings and citations cannot stand in for human
acceptance or claim verification.

## Harness decision

The trace shows useful evidence was already available before the workflow kept
investigating, compacted, and re-read overlapping files. The next harness change
is a Repo-Brief-specific evidence-call budget that removes tools once consumed,
allows natural response completion without a turn cap, preserves gathered
evidence longer before compaction, and reports the budget in the output
contract.

The initial 16-call budget is a trace-derived product hypothesis, not a
calibrated universal optimum. It must be evaluated across the measured beta task
set for contract completion, human acceptance, latency, and cases where the
budget blocks evidence that would have changed the answer.

No repeat provider run is planned until that deterministic change passes local
tests. The single run exceeded the expected latency and usage envelope.
