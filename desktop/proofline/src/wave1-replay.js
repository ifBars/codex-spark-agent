export const WAVE1_EVENT_SCHEMA = "spark.proofline.validation.v1";

const EVENT_TYPES = new Set([
  "app_ready",
  "run_submitted",
  "activity_rendered",
  "approval_presented",
  "approval_decided",
  "transcript_evidence_opened",
  "diff_opened",
  "validation_reported",
  "checkpoint_actioned",
  "usage_viewed",
  "task_outcome",
  "privacy_boundary_checked",
]);

const RUN_STATES = new Set(["starting", "running", "awaiting_approval", "completed", "failed", "cancelled"]);
const SURFACES = new Set(["rail", "transcript", "evidence_drawer", "composer", "status_ribbon"]);
const RESULTS = new Set(["success", "failure", "abandoned", "hinted"]);
const TASK_IDS = new Set(["proofline-1", "proofline-2", "proofline-3", "proofline-4", "proofline-5"]);
const APPROVAL_KINDS = new Set(["none", "file_change", "command"]);
const VALIDATION_STATES = new Set(["unknown", "not_run", "passed", "failed"]);
const USAGE_STATES = new Set(["unavailable", "partial", "authoritative"]);
const PRICING_STATES = new Set(["unavailable", "available"]);

const replayTasks = [
  {
    id: "repo-brief",
    title: "Read a Repo Brief",
    summary: "Identify the likely ownership boundary, inspect its cited evidence, and keep one uncertainty visible.",
    time: "Replay", elapsed: "Ready", complete: false,
    scenario: "repo-brief",
  },
  {
    id: "completed-change",
    title: "Inspect a completed change",
    summary: "Review a completed two-file fixture change with a passed validation record and checkpoint.",
    time: "Replay", elapsed: "Ready", complete: true,
    scenario: "completed-change",
  },
  {
    id: "failed-validation",
    title: "Recover from failed validation",
    summary: "Distinguish a failed fixture validation from completion, then choose an informed simulated recovery action.",
    time: "Replay", elapsed: "Ready", complete: false,
    scenario: "failed-validation",
  },
  {
    id: "pending-approval",
    title: "Resolve an approval",
    summary: "Apply the printed policy to a pending simulated authority request rather than treating the run as stalled.",
    time: "Replay", elapsed: "Ready", complete: false,
    scenario: "pending-approval",
  },
  {
    id: "partial-usage",
    title: "Interpret usage and hand off",
    summary: "Read source-reported usage honestly: coverage is partial and pricing is unavailable.",
    time: "Replay", elapsed: "Ready", complete: true,
    scenario: "partial-usage",
  },
];

export const wave1TaskGroup = { label: "Wave 1 fixture", tasks: replayTasks };

export const WAVE1_FIXTURE_ID = "proofline-wave1-local";
export const WAVE1_FIXTURE_REVISION = "2026-08-01.1";
export const wave1FixtureManifest = Object.freeze({
  schema: "spark.proofline.fixture.v1",
  fixture_id: WAVE1_FIXTURE_ID,
  revision: WAVE1_FIXTURE_REVISION,
  runtime_mode: "replayed",
  evidence_files: [{ path: "fixtures/ownership-map.md", sha256: "84ba388c94579204b6ca1861a42d8f1ceae851d0dc06fd9cfc59d9a966112c93" }],
  scenarios: [
    { task_id: "proofline-1", outcome: "cited-evidence-with-uncertainty" },
    { task_id: "proofline-2", outcome: "two-files-passed-validation-checkpoint" },
    { task_id: "proofline-3", outcome: "failed-validation-recovery-choice" },
    { task_id: "proofline-4", outcome: "pending-file-change-approval" },
    { task_id: "proofline-5", outcome: "partial-usage-pricing-unavailable" },
  ],
});

// This is the SHA-256 of the canonical JSON manifest above. It is a fixture
// identity display, not a claim that the browser has verified an archive.
export const WAVE1_FIXTURE_SHA256 = "7829776e9aea00a0d182d00cddc3337f07659d728fbea9b31b30fdc05f36b3bf";

export function canonicalWave1FixtureManifest() {
  return JSON.stringify(wave1FixtureManifest);
}

export const wave1ScenarioFixtures = {
  "repo-brief": {
    taskId: "proofline-1",
    runState: "completed",
    citation: { source: "fixtures/ownership-map.md:14", excerpt: "The parser owns normalization before the routing boundary." },
    answer: "The parser module is the likely ownership boundary.",
    uncertainty: "The replay does not establish whether downstream callers bypass normalization.",
    nextCheck: "Inspect the routing fixture call sites.",
  },
  "completed-change": {
    taskId: "proofline-2",
    runState: "completed",
    files: ["fixture/usage/history.rs", "fixture/tests/usage_history_fork.rs"],
    validation: "Passed fixture validation",
    checkpoint: "fixture-checkpoint-a",
  },
  "failed-validation": {
    taskId: "proofline-3",
    runState: "failed",
    validation: "Failed fixture validation",
    failureCommand: "fixture validate --task proofline-3",
    failureOutput: "Fixture assertion failed: expected result did not match.",
    failure: "The deterministic fixture test did not meet its expected assertion.",
    choices: ["Inspect simulated diff", "Retry simulated validation", "Restore simulated checkpoint"],
    checkpoint: "fixture-checkpoint-b",
  },
  "pending-approval": {
    taskId: "proofline-4",
    runState: "awaiting_approval",
    approvalKind: "file_change",
    policy: "Approve fixture-only file changes. Deny command authority in this replay.",
  },
  "partial-usage": {
    taskId: "proofline-5",
    runState: "completed",
    usageState: "partial",
    pricingState: "unavailable",
    sourceReportedTokens: { input: 18742, output: 4396 },
    handoff: "Share the evidence record with the explicit partial-coverage and unavailable-pricing labels.",
  },
};

export function getWave1Scenario(id) {
  return wave1ScenarioFixtures[id] ?? wave1ScenarioFixtures["repo-brief"];
}

export function initialWave1ReplayState(runEpoch = 1) {
  const sessionId = `wave1-local-fixture-${runEpoch}`;
  const appReady = createWave1Interaction({
    sequence: 1,
    runEpoch,
    sessionId,
    taskId: "proofline-1",
    eventType: "app_ready",
    runState: "starting",
    surface: "rail",
  });
  return {
    runEpoch,
    sessionId,
    runStarted: false,
    openedRepoEvidence: false,
    openedDiff: false,
    recoveryChoice: null,
    approvalDecision: null,
    usageViewed: false,
    exportViewed: false,
    purged: false,
    events: [appReady],
    nextSequence: 2,
  };
}

function allowedMetadata(taskId, metadata = {}) {
  const allowed = {
    task_id: taskId,
    approval_kind: metadata.approval_kind ?? "none",
    validation_state: metadata.validation_state ?? "unknown",
    usage_state: metadata.usage_state ?? "unavailable",
    pricing_state: metadata.pricing_state ?? "unavailable",
  };
  if (!isAllowedMetadata(allowed)) throw new Error("Unsupported Wave 1 metadata category");
  return allowed;
}

function isAllowedMetadata(metadata) {
  return TASK_IDS.has(metadata.task_id)
    && APPROVAL_KINDS.has(metadata.approval_kind)
    && VALIDATION_STATES.has(metadata.validation_state)
    && USAGE_STATES.has(metadata.usage_state)
    && PRICING_STATES.has(metadata.pricing_state);
}

export function createWave1Interaction({ sequence, runEpoch = 1, sessionId = "wave1-local-fixture-1", eventType, runState, surface, result = "success", taskId, metadata, latencyMs = 0 }) {
  if (!EVENT_TYPES.has(eventType)) throw new Error(`Unsupported Wave 1 event type: ${eventType}`);
  if (!RUN_STATES.has(runState)) throw new Error(`Unsupported Wave 1 run state: ${runState}`);
  if (!SURFACES.has(surface)) throw new Error(`Unsupported Wave 1 surface: ${surface}`);
  if (!RESULTS.has(result)) throw new Error(`Unsupported Wave 1 result: ${result}`);
  if (!Number.isInteger(sequence) || sequence < 1 || !Number.isInteger(runEpoch) || runEpoch < 1) throw new Error("Invalid Wave 1 event identity");
  if (sessionId !== `wave1-local-fixture-${runEpoch}`) throw new Error("Invalid Wave 1 session identity");

  return {
    schema: WAVE1_EVENT_SCHEMA,
    event_id: `wave1-local-${runEpoch}-${sequence}`,
    occurred_at: new Date(Date.UTC(2026, 7, 1, 0, 0, sequence)).toISOString(),
    session_id: sessionId,
    participant_id: null,
    thread_id: `fixture-${taskId}`,
    sequence,
    event_type: eventType,
    run_state: runState,
    surface,
    latency_ms: latencyMs,
    result,
    metadata: allowedMetadata(taskId, metadata),
  };
}

function record(state, scenario, input) {
  const event = createWave1Interaction({ sequence: state.nextSequence, runEpoch: state.runEpoch, sessionId: state.sessionId, taskId: scenario.taskId, runState: scenario.runState, ...input });
  return { ...state, nextSequence: state.nextSequence + 1, events: [...state.events, event] };
}

export function reduceWave1ReplayState(state, action) {
  const scenario = getWave1Scenario(action.scenario);

  switch (action.type) {
    case "start-replay": {
      const submitted = record({ ...state, runStarted: true, purged: false }, scenario, {
        eventType: "run_submitted", runState: "starting", surface: "composer", metadata: { validation_state: "not_run" },
      });
      return record(submitted, scenario, {
        eventType: "activity_rendered", runState: "running", surface: "transcript", latencyMs: 200, metadata: { validation_state: "not_run" },
      });
    }
    case "open-repo-evidence":
      return record({ ...state, openedRepoEvidence: true }, scenario, {
        eventType: "transcript_evidence_opened", surface: "transcript", metadata: { validation_state: "not_run" },
      });
    case "open-diff":
      return record({ ...state, openedDiff: true }, scenario, {
        eventType: "diff_opened", surface: "evidence_drawer", metadata: { validation_state: "passed" },
      });
    case "recover":
      return record({ ...state, recoveryChoice: action.choice }, scenario, {
        eventType: action.choice === "Restore simulated checkpoint" ? "checkpoint_actioned" : "validation_reported",
        surface: "evidence_drawer", result: "success", metadata: { validation_state: "failed" },
      });
    case "decide-approval":
      return record({ ...state, approvalDecision: action.decision }, scenario, {
        eventType: "approval_decided", surface: "evidence_drawer", result: action.decision === "approve" ? "success" : "abandoned",
        metadata: { approval_kind: scenario.approvalKind, validation_state: "not_run" },
      });
    case "view-usage":
      return record({ ...state, usageViewed: true }, scenario, {
        eventType: "usage_viewed", surface: "status_ribbon", metadata: { usage_state: "partial", pricing_state: "unavailable" },
      });
    case "view-redacted-export":
      return { ...state, exportViewed: true };
    case "reset-local-replay":
      return initialWave1ReplayState(state.runEpoch + 1);
    case "purge-local-events": {
      const nextRun = initialWave1ReplayState(state.runEpoch + 1);
      return { ...nextRun, events: [], nextSequence: 1, purged: true };
    }
    default:
      return state;
  }
}

export function createRedactedWave1Export(events) {
  const byTask = new Map();
  for (const event of events) {
    const taskId = event.metadata.task_id;
    byTask.set(taskId, (byTask.get(taskId) ?? 0) + 1);
  }
  return {
    schema: "spark.proofline.validation.aggregate.v1",
    fixture: { id: WAVE1_FIXTURE_ID, revision: WAVE1_FIXTURE_REVISION, sha256: WAVE1_FIXTURE_SHA256 },
    event_count: events.length,
    event_counts_by_task: [...byTask.entries()].map(([task_id, count]) => ({ task_id, count })),
    redaction: "aggregate-only",
  };
}

export function isPrivacySafeWave1Event(event) {
  const eventKeys = ["schema", "event_id", "occurred_at", "session_id", "participant_id", "thread_id", "sequence", "event_type", "run_state", "surface", "latency_ms", "result", "metadata"];
  const metadataKeys = ["task_id", "approval_kind", "validation_state", "usage_state", "pricing_state"];
  if (Object.keys(event).some((key) => !eventKeys.includes(key))) return false;
  if (Object.keys(event.metadata ?? {}).some((key) => !metadataKeys.includes(key))) return false;
  if (!EVENT_TYPES.has(event.event_type) || !RUN_STATES.has(event.run_state) || !SURFACES.has(event.surface) || !RESULTS.has(event.result)) return false;
  if (!isAllowedMetadata(event.metadata ?? {})) return false;
  if (!/^wave1-local-\d+-\d+$/.test(event.event_id) || !/^wave1-local-fixture-\d+$/.test(event.session_id)) return false;
  if (event.participant_id !== null || event.thread_id !== `fixture-${event.metadata.task_id}`) return false;

  // `diff_opened` is an allowed categorical event name; the fixture never logs diff content.
  const values = [event.event_id, event.session_id, event.thread_id, ...Object.values(event.metadata ?? {})]
    .filter((value) => typeof value === "string")
    .join(" ")
    .toLowerCase();
  return !["http", "@", "credential", "password", "secret", "prompt", "command"].some((term) => values.includes(term));
}
