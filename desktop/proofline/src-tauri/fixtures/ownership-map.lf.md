# Proofline ownership-map fixture

This file is deterministic evidence for the local Wave 1 replay only.

## Candidate modules

- `routing` consumes normalized requests and selects a handler.
- `parser` accepts fixture input and normalizes it before routing.

## Ownership boundary

Evidence recorded for the Repo Brief task:

The parser owns normalization before the routing boundary.

Uncertainty: downstream fixture callers may bypass normalization.
