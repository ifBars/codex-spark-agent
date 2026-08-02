//! Fixture-only state and presentation mapping for the native Proofline spike.
//!
//! This crate intentionally contains no authority-bearing integration points.

mod fixture;
mod model;
mod presentation;

pub use fixture::fixture_snapshot;
pub use model::{ProoflineSnapshotV1, RunState};
pub use presentation::{ProoflinePresentation, StatusRibbon};
