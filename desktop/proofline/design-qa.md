# Proofline design QA

Status: passed after truth-boundary and responsive remediation on 2026-08-01. No open P0, P1, or P2 visual or core-interaction defects.

## Target and capture conditions

- Selected source: `reference-proofline.png`, native 1487 x 1058.
- Implementation: `implementation-final.png`, captured at 1600 x 900 in the in-app browser at its default desktop density.
- Comparison: `design-qa-comparison.png`. The source is preserved at 1:1 scale, right-padded with the source background, and clipped to the same 1600 x 900 top-fold analysis canvas as the implementation. Both sides show the initial completed-task state.
- Preview: `http://127.0.0.1:4173/`.

## QA gates

| Gate | Result | Evidence |
| --- | --- | --- |
| Outcome-first hierarchy | Pass | Completion, title, summary, review action, file ledger, validation, work trace, composer, and status ribbon follow the selected concept in the same order. |
| Desktop geometry | Pass | The fresh shared top-fold comparison aligns the rail seam, main content origin, title baseline, file ledger width, validation stack, work-trace row, and composer while preserving the visible prototype boundary. |
| Core interactions | Pass | Browser replay and focused state tests cover task switching, unavailable evidence, file-inspector open/focus/close, authority labels, and the prototype-only composer submission notice. |
| Responsive behavior | Pass | Rechecked at 1024, 800, and approximately 451 CSS px. The document and task surface stay horizontally contained; the narrow task rail and composer controls scroll within their own regions. |
| Content integrity | Pass | The renderer visibly labels simulated data, does not reuse the fork fixture for other tasks, keeps unavailable evidence explicit, and never estimates price or claims a sandbox. |
| Build and site tests | Pass | `bun run test:app`, `bun run build`, and `bun run test:sites` pass. |

## Accepted low-priority differences

- P3: the Spark mark uses the closest matching Phosphor icon because the concept did not include a reusable brand asset.
- P3: the completion summary and status ribbon explicitly identify the rich state as a simulated fixture; this is an intentional product-truth correction to the mock copy.
- P3: the mode selector says `Ask (read-only tools)` or `Work (OS-user access)` and gives a visible no-sandbox/no-privacy-guarantee note instead of a private or full-access promise.

## Artifacts

- `reference-proofline.png`: selected concept.
- `implementation-final.png`: verified implementation state.
- `reference-qa-1600x900.png`: deterministic analysis canvas for the source top fold.
- `design-qa-comparison.png`: final side-by-side comparison.
- `qa-comparison.html`: reproducible comparison page.
- `audit-01-initial.png`: canonical completed-task state.
- `audit-02-file-inspector.png`: simulated file-inspector interaction.
- `audit-03-unavailable-evidence.png`: non-fork task with no inherited evidence.
- `audit-04-authority-and-submit.png`: Work-mode authority and prototype-only submission notice.
- `audit-05-mobile.png`: narrow responsive state.
