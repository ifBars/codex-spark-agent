# Prototype Instructions

Run the local server yourself and open the preview in the browser available to this environment. Do not give the user server-start instructions when you can run it.

Before making substantial visual changes, use the Product Design plugin's `get-context` skill when the visual source is unclear or no longer matches the current goal. When the user gives durable prototype-specific design feedback, preferences, or decisions, record them in `AGENTS.md`.

When implementing from a selected generated mock, treat that image as the source of truth for layout, component anatomy, density, spacing, color, typography, visible content, and hierarchy.

Build app UI in `src/`. Keep `.openai/hosting.json`, `worker/index.js`, `scripts/prepare-sites-build.mjs`, and `tests/sites-worker.test.mjs` intact so the same local prototype can be handed to Sites. Before a Sites handoff, run `bun run build` and `bun run test:sites`; the build must leave `dist/client/index.html`, `dist/server/index.js`, and `dist/.openai/hosting.json`.

## Spark Bench visual direction

- Do not use a generic dark dashboard, sidebar filter rail, card mosaic, glassmorphism, glow, particle field, or purple/blue SaaS styling.
- Ground the UI in the user's existing work:
  - `C:\Users\ghost\Desktop\Coding\React\stackbench` for warm editorial surfaces, ledger rules, Geist/Outfit/mono typography, and benchmark data presentation.
  - `C:\Users\ghost\Desktop\Coding\React\movie-wizard` for the charcoal header, compact controls, orange active state, and responsive collapse.
  - `C:\Users\ghost\Desktop\Coding\React\codalyn` for technical precision and inspector discipline only; do not copy its decorative orbs or glow effects.
- Use a warm mineral canvas, near-black header, mostly cardless full-width bands, thin borders, compact segmented controls, and tabular numeric columns.
- The chart is the primary object. Filters support it; they must not become a separate dashboard.
