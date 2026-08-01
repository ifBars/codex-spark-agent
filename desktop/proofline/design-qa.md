# Proofline design QA

Status: passed on 2026-08-01. No open P0, P1, or P2 visual or interaction defects.

## Target and capture conditions

- Selected source: `reference-proofline.png`, native 1487 x 1058.
- Implementation: `implementation-final.png`, captured at 1600 x 900 in the in-app browser at its default desktop density.
- Comparison: `design-qa-comparison.png`. The source is preserved at 1:1 scale, right-padded with the source background, and clipped to the same 1600 x 900 top-fold analysis canvas as the implementation. Both sides show the initial completed-task state.
- Preview: `http://127.0.0.1:4173/`.

## QA gates

| Gate | Result | Evidence |
| --- | --- | --- |
| Outcome-first hierarchy | Pass | Completion, title, summary, review action, file ledger, validation, work trace, composer, and status ribbon follow the selected concept in the same order. |
| Desktop geometry | Pass | Shared top-fold comparison aligns the 294 px rail seam, main content origin, title baseline, file ledger width, validation stack, work-trace row, and composer. |
| Core interactions | Pass | Review open/close, continue-to-composer, file drawer toggle, work-trace expansion, model/reasoning/workspace selection, prompt submission, and task switching were exercised through accessible controls. |
| Responsive behavior | Pass | Checked at 1024, 800, and approximately 451 CSS px. The mobile rail remains horizontally scrollable inside its own region and the document no longer overflows horizontally. |
| Content integrity | Pass | Usage copy is limited to source-reported tokens. Pricing is explicitly unavailable rather than estimated or invented. |
| Build and site tests | Pass | `bun run build` and `bun run test:sites` pass. |

## Accepted low-priority differences

- P3: the Spark mark uses the closest matching Phosphor icon because the concept did not include a reusable brand asset.
- P3: the completion summary includes the pricing-unavailable boundary; this is an intentional product-truth correction to the mock copy.
- P3: the status ribbon says `Permissions shown` instead of `Private`; the renderer cannot make a security promise that the harness and provider path do not yet prove.

## Artifacts

- `reference-proofline.png`: selected concept.
- `implementation-final.png`: verified implementation state.
- `reference-qa-1600x900.png`: deterministic analysis canvas for the source top fold.
- `design-qa-comparison.png`: final side-by-side comparison.
- `qa-comparison.html`: reproducible comparison page.
