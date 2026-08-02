# Prototype Instructions

Run the local server yourself and open the preview in the browser available to this environment. Do not give the user server-start instructions when you can run it.

Before making substantial visual changes, use the Product Design plugin's `get-context` skill when the visual source is unclear or no longer matches the current goal. When the user gives durable prototype-specific design feedback, preferences, or decisions, record them in `AGENTS.md`.

When implementing from a selected generated mock, treat that image as the source of truth for layout, component anatomy, density, spacing, color, typography, visible content, and hierarchy.

The selected Proofline desktop concept with a task-history rail, evidence-first completed-task detail, anchored composer controls, and persistent token/pricing/privacy status is the current product reference. Preserve that hierarchy as native behavior replaces rehearsal data.

## Operational surface direction

Proofline should open on a real product-direction task, not a fixture or facilitator workflow. Keep the primary surface compact and calm: a grouped task rail, concise completed-task summary, file and validation ledger, optional side-by-side review pane, anchored composer, and one-line branch/checkpoint/usage/privacy ribbon. Keep the warm Proofline identity; avoid decorative selection rails, repeated cards, badges used as decoration, oversized editorial headings at ordinary desktop widths, and fixture disclaimers in the main task flow. Rehearsal, measurement, export, reset, purge, and other administrative controls remain functional but belong behind one clearly labelled advanced disclosure.

Build app UI in `src/`. Keep `.openai/hosting.json`, `worker/index.js`, `scripts/prepare-sites-build.mjs`, and `tests/sites-worker.test.mjs` intact so the same local prototype can be handed to Sites. Before a Sites handoff, run `bun run build` and `bun run test:sites`; the build must leave `dist/client/index.html`, `dist/server/index.js`, and `dist/.openai/hosting.json`.
