# Proofline for Spark

Proofline is an interactive product prototype for a local evidence-and-review surface over the Spark harness. It preserves the selected outcome-first concept: recent tasks in the rail, a completed outcome, changed-file and validation evidence, a collapsible work trace, an instruction composer, and visible usage and authority boundaries.

This directory is a renderer prototype with realistic mock data. It is not connected to the Rust agent loop, session store, filesystem, tools, pricing, or provider account. The proposed integration boundary is specified in [proofline-event-contract.md](../../docs/product/proofline-event-contract.md), and the user-validation gate is specified in [proofline-validation.md](../../docs/product/proofline-validation.md).

## Local preview

```powershell
bun install
bun run dev
```

Use `bun run build` and `bun run test:sites` for the verified production-shaped build and worker checks.

## Prototype truth boundaries

- Token values are presented as source-reported observations, not billing.
- Pricing stays unavailable until Spark has a versioned price source and methodology.
- `Permissions shown` describes the visible authority control; it is not an OS-sandbox or privacy guarantee.
- Changed files, checkpoints, validation, and work steps are illustrative until typed backend records exist.
