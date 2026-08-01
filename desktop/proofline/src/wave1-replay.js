export const WAVE1_FIXTURE_ID = "proofline-wave1-local";
export const WAVE1_FIXTURE_REVISION = "2026-08-01.1";
export const WAVE1_FIXTURE_SHA256 = "29415e0ce8f7659093e01032ce52365197c59d0010bad7aa4361048fdb86abe5";

const replayTasks = [
  { id: "repo-brief", title: "Read a Repo Brief", summary: "Identify the likely ownership boundary and inspect its cited evidence.", time: "Replay", elapsed: "Ready", complete: false, scenario: "repo-brief" },
  { id: "completed-change", title: "Inspect a completed change", summary: "Review a completed two-file fixture change with passed validation.", time: "Replay", elapsed: "Ready", complete: true, scenario: "completed-change" },
  { id: "failed-validation", title: "Recover from failed validation", summary: "Distinguish a failed fixture validation from completion.", time: "Replay", elapsed: "Ready", complete: false, scenario: "failed-validation" },
  { id: "pending-approval", title: "Resolve an approval", summary: "Apply a printed policy to a pending fixture authority request.", time: "Replay", elapsed: "Ready", complete: false, scenario: "pending-approval" },
  { id: "partial-usage", title: "Interpret usage and hand off", summary: "Read source-reported usage honestly: coverage is partial and pricing unavailable.", time: "Replay", elapsed: "Ready", complete: true, scenario: "partial-usage" },
];

export const wave1TaskGroup = { label: "Wave 1 fixture", tasks: replayTasks };
export const wave1FixtureManifest = Object.freeze({ schema: "spark.proofline.fixture.v1", fixture_id: WAVE1_FIXTURE_ID, revision: WAVE1_FIXTURE_REVISION, runtime_mode: "replayed", evidence_files: [{ path: "fixtures/ownership-map.md", sha256: "84ba388c94579204b6ca1861a42d8f1ceae851d0dc06fd9cfc59d9a966112c93" }] });
export function canonicalWave1FixtureManifest() { return JSON.stringify(wave1FixtureManifest); }

export const wave1ScenarioFixtures = {
  "repo-brief": { taskId: "proofline-1", citation: { source: "fixtures/ownership-map.md:14", excerpt: "The parser owns normalization before the routing boundary." }, answer: "The parser module is the likely ownership boundary.", uncertainty: "The rehearsal does not establish whether downstream callers bypass normalization.", nextCheck: "Inspect routing fixture call sites." },
  "completed-change": { taskId: "proofline-2", files: ["fixture/usage/history.rs", "fixture/tests/usage_history_fork.rs"], validation: "Passed fixture validation", checkpoint: "fixture-checkpoint-a" },
  "failed-validation": { taskId: "proofline-3", validation: "Failed fixture validation", failureCommand: "fixture validate --task proofline-3", failureOutput: "Fixture assertion failed: expected result did not match.", failure: "The deterministic fixture test did not meet its expected assertion.", choices: ["Inspect simulated diff", "Retry simulated validation", "Restore simulated checkpoint"], checkpoint: "fixture-checkpoint-b" },
  "pending-approval": { taskId: "proofline-4", approvalKind: "file_change", policy: "Approve fixture-only file changes. Deny command authority in this rehearsal." },
  "partial-usage": { taskId: "proofline-5", usageState: "partial", pricingState: "unavailable", sourceReportedTokens: { input: 18742, output: 4396 }, handoff: "Share the aggregate with partial-coverage and unavailable-pricing labels." },
};

export function getWave1Scenario(id) { return wave1ScenarioFixtures[id] ?? wave1ScenarioFixtures["repo-brief"]; }
export function wave1FixtureRequest() { return { id: WAVE1_FIXTURE_ID, revision: WAVE1_FIXTURE_REVISION, sha256: WAVE1_FIXTURE_SHA256 }; }
export const initialWave1ReplayViewState = Object.freeze({ runStarted: false, openedRepoEvidence: false, openedDiff: false, recoveryChoice: null, approvalDecision: null, usageViewed: false });
export function reduceWave1ReplayViewState(state, action) {
  switch (action.type) {
    case "start": return { ...state, runStarted: true };
    case "open-evidence": return { ...state, openedRepoEvidence: true };
    case "open-change": return { ...state, openedDiff: true };
    case "recover": return { ...state, recoveryChoice: action.choice };
    case "decide": return { ...state, approvalDecision: action.decision };
    case "usage": return { ...state, usageViewed: true };
    default: return state;
  }
}
