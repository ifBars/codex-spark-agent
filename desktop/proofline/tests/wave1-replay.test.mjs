import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import test from "node:test";
import {
  WAVE1_EVENT_SCHEMA,
  WAVE1_FIXTURE_SHA256,
  canonicalWave1FixtureManifest,
  createWave1Interaction,
  createRedactedWave1Export,
  getWave1Scenario,
  initialWave1ReplayState,
  isPrivacySafeWave1Event,
  reduceWave1ReplayState,
  wave1FixtureManifest,
  wave1TaskGroup,
} from "../src/wave1-replay.js";

function act(state, type, scenario, extra = {}) {
  return reduceWave1ReplayState(state, { type, scenario, ...extra });
}

test("Wave 1 exposes all five documented replay tasks with their executable evidence states", () => {
  assert.equal(wave1TaskGroup.tasks.length, 5);
  assert.deepEqual(wave1TaskGroup.tasks.map((task) => task.scenario), [
    "repo-brief", "completed-change", "failed-validation", "pending-approval", "partial-usage",
  ]);
  assert.match(getWave1Scenario("repo-brief").citation.source, /:\d+$/);
  assert.equal(getWave1Scenario("completed-change").files.length, 2);
  assert.equal(getWave1Scenario("completed-change").validation, "Passed fixture validation");
  assert.equal(getWave1Scenario("failed-validation").runState, "failed");
  assert.match(getWave1Scenario("failed-validation").failureCommand, /^fixture validate/);
  assert.match(getWave1Scenario("failed-validation").failureOutput, /assertion failed/i);
  assert.deepEqual(getWave1Scenario("failed-validation").choices, ["Inspect simulated diff", "Retry simulated validation", "Restore simulated checkpoint"]);
  assert.equal(getWave1Scenario("pending-approval").runState, "awaiting_approval");
  assert.equal(getWave1Scenario("partial-usage").usageState, "partial");
  assert.equal(getWave1Scenario("partial-usage").pricingState, "unavailable");
  assert.deepEqual(getWave1Scenario("partial-usage").sourceReportedTokens, { input: 18742, output: 4396 });
});

test("the replay fixture has a pinned manifest revision, evidence file, and SHA-256 representation", async () => {
  const actual = createHash("sha256").update(canonicalWave1FixtureManifest()).digest("hex");
  assert.equal(actual, WAVE1_FIXTURE_SHA256);
  const evidence = await readFile(new URL("../fixtures/ownership-map.md", import.meta.url));
  assert.equal(createHash("sha256").update(evidence).digest("hex"), wave1FixtureManifest.evidence_files[0].sha256);
  assert.equal(evidence.toString("utf8").split(/\r?\n/)[13], getWave1Scenario("repo-brief").citation.excerpt);
});

test("replay interactions are local, versioned, monotonic, and free of disallowed content", () => {
  let state = initialWave1ReplayState();
  state = act(state, "start-replay", "repo-brief");
  state = act(state, "open-repo-evidence", "repo-brief");
  state = act(state, "open-diff", "completed-change");
  state = act(state, "recover", "failed-validation", { choice: "Restore simulated checkpoint" });
  state = act(state, "decide-approval", "pending-approval", { decision: "approve" });
  state = act(state, "view-usage", "partial-usage");

  assert.deepEqual(state.events.map((event) => event.sequence), [1, 2, 3, 4, 5, 6, 7, 8]);
  assert.deepEqual(state.events.map((event) => event.event_id), ["wave1-local-1-1", "wave1-local-1-2", "wave1-local-1-3", "wave1-local-1-4", "wave1-local-1-5", "wave1-local-1-6", "wave1-local-1-7", "wave1-local-1-8"]);
  assert.equal(state.events[0].event_type, "app_ready");
  assert.equal(state.events[1].event_type, "run_submitted");
  assert.equal(state.events[2].event_type, "activity_rendered");
  assert.equal(state.events[2].latency_ms, 200);
  for (const event of state.events) {
    assert.equal(event.schema, WAVE1_EVENT_SCHEMA);
    assert.equal(event.session_id, "wave1-local-fixture-1");
    assert.equal(event.participant_id, null);
    assert.equal(isPrivacySafeWave1Event(event), true);
    assert.deepEqual(Object.keys(event.metadata).sort(), ["approval_kind", "pricing_state", "task_id", "usage_state", "validation_state"]);
  }
});

test("reset, purge, and redacted export remain local fixture operations", () => {
  let state = initialWave1ReplayState();
  state = act(state, "start-replay", "repo-brief");
  state = act(state, "open-repo-evidence", "repo-brief");
  const exported = createRedactedWave1Export(state.events);
  assert.equal(exported.redaction, "aggregate-only");
  assert.equal(exported.event_count, 4);
  assert.deepEqual(Object.keys(exported).sort(), ["event_count", "event_counts_by_task", "fixture", "redaction", "schema"]);
  assert.doesNotMatch(JSON.stringify(exported), /wave1-local-|occurred_at|session_id|fixture\/ownership|command|prompt|token/i);

  const reset = act(state, "reset-local-replay", "repo-brief");
  assert.equal(reset.events.length, 1);
  assert.equal(reset.events[0].event_type, "app_ready");
  assert.equal(reset.runEpoch, 2);
  assert.equal(reset.sessionId, "wave1-local-fixture-2");
  assert.equal(reset.events[0].event_id, "wave1-local-2-1");
  assert.notEqual(reset.events[0].event_id, state.events[0].event_id);
  const purged = act(state, "purge-local-events", "repo-brief");
  assert.equal(purged.events.length, 0);
  assert.equal(purged.nextSequence, 1);
  assert.equal(purged.runEpoch, state.runEpoch + 1);
  assert.equal(purged.sessionId, "wave1-local-fixture-2");
  assert.equal(purged.purged, true);

  const afterPurge = act(purged, "open-repo-evidence", "repo-brief");
  assert.equal(afterPurge.events[0].event_id, "wave1-local-2-1");
  assert.notEqual(afterPurge.events[0].event_id, state.events[0].event_id);
});

test("recovery and approval actions change only simulated local replay state", () => {
  const recovered = act(initialWave1ReplayState(), "recover", "failed-validation", { choice: "Retry simulated validation" });
  assert.equal(recovered.recoveryChoice, "Retry simulated validation");
  assert.equal(recovered.events[1].run_state, "failed");
  assert.equal(recovered.events[1].metadata.validation_state, "failed");

  const denied = act(initialWave1ReplayState(), "decide-approval", "pending-approval", { decision: "deny" });
  assert.equal(denied.approvalDecision, "deny");
  assert.equal(denied.events[1].event_type, "approval_decided");
  assert.equal(denied.events[1].metadata.approval_kind, "file_change");
  assert.equal(denied.events[1].result, "abandoned");
});

test("event construction rejects unrecognized event categories rather than accepting arbitrary payloads", () => {
  assert.throws(() => createWave1Interaction({
    sequence: 1,
    taskId: "proofline-1",
    eventType: "network_posted",
    runState: "completed",
    surface: "transcript",
  }), /Unsupported Wave 1 event type/);
  assert.throws(() => createWave1Interaction({
    sequence: 1,
    taskId: "proofline-1",
    eventType: "approval_decided",
    runState: "completed",
    surface: "transcript",
    metadata: { approval_kind: "C:/private/customer/repo" },
  }), /Unsupported Wave 1 metadata category/);
});
