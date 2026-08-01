import assert from "node:assert/strict";
import test from "node:test";
import { createWave1Adapter, isParticipantId, validateAggregate, validateHostAcknowledgement, validateHostPreflight, validateHostSession, validateInteraction } from "../src/wave1-bridge.js";
import { initialWave1MeasurementState, reduceWave1MeasurementState } from "../src/wave1-ledger.js";
import { getWave1Scenario, initialWave1ReplayViewState, reduceWave1ReplayViewState, wave1TaskGroup } from "../src/wave1-replay.js";

const preflight = { countable: true, reason: null, fixtureManifestSha256: "a".repeat(64), evidenceVerified: true, build: { gitSha: "b".repeat(40), dirty: false } };
const session = { sessionId: "opaque-host-value", namespace: "wave1-12345678", startedAtMs: 17, nextSequence: 1, preflight };
const aggregate = { schema: "spark.proofline.validation.v1", countable: true, eventCount: 8, eventCountsByTask: [{ taskId: "proofline-1", count: 2 }], eventCountsByCategory: [{ interaction: "recovery_selected", count: 1 }], sessionNamespaceRotated: false };

test("all five rich rehearsal task states remain accessible", () => {
  assert.equal(wave1TaskGroup.tasks.length, 5);
  const repo = getWave1Scenario("repo-brief"); const change = getWave1Scenario("completed-change"); const failed = getWave1Scenario("failed-validation"); const approval = getWave1Scenario("pending-approval"); const usage = getWave1Scenario("partial-usage");
  assert.match(repo.citation.source, /:\d+$/); assert.equal(change.files.length, 2); assert.equal(change.validation, "Passed fixture validation"); assert.match(failed.failureCommand, /^fixture validate/); assert.equal(failed.choices.length, 3); assert.match(approval.policy, /Approve fixture-only/i); assert.deepEqual(usage.sourceReportedTokens, { input: 18742, output: 4396 }); assert.equal(usage.pricingState, "unavailable");
});

test("visible replay state changes are local display state with no generated event identity or time", () => {
  let state = initialWave1ReplayViewState;
  state = reduceWave1ReplayViewState(state, { type: "start" }); state = reduceWave1ReplayViewState(state, { type: "open-evidence" }); state = reduceWave1ReplayViewState(state, { type: "open-change" }); state = reduceWave1ReplayViewState(state, { type: "recover", choice: "Restore simulated checkpoint" }); state = reduceWave1ReplayViewState(state, { type: "decide", decision: "approve" }); state = reduceWave1ReplayViewState(state, { type: "usage" });
  assert.deepEqual(state, { runStarted: true, openedRepoEvidence: true, openedDiff: true, recoveryChoice: "Restore simulated checkpoint", approvalDecision: "approve", usageViewed: true });
  assert.doesNotMatch(JSON.stringify(state), /event_id|timestamp|occurred_at|sequence|session/i);
});

test("browser adapter fails closed as a non-countable rehearsal without a host invocation", async () => {
  const adapter = createWave1Adapter({ hostAvailable: false });
  assert.equal(adapter.kind, "browser"); assert.equal((await adapter.preflight()).countable, false); assert.equal((await adapter.startSession()).preflight.countable, false); assert.equal((await adapter.appendInteraction({ taskId: "proofline-1", interaction: "replay_submitted" })).accepted, false); assert.equal((await adapter.previewAggregate()).eventCount, 0);
});

test("host adapter uses only host contract commands and display content never enters DTOs", async () => {
  const calls = [];
  const invoke = async (command, args) => { calls.push({ command, args }); if (command === "wave1_preflight") return preflight; if (command === "wave1_start_session") return session; if (command === "wave1_append_event") return { accepted: true, sequence: 2, timestampMs: 30 }; if (command === "wave1_preview_aggregate") return aggregate; return preflight; };
  const adapter = createWave1Adapter({ invoke }); await adapter.preflight(); await adapter.startSession(); await adapter.appendInteraction({ taskId: "proofline-4", interaction: "approval_decided", approvalDecision: "approved" }); await adapter.previewAggregate(); await adapter.purgeSession();
  assert.deepEqual(calls.map(({ command }) => command), ["wave1_preflight", "wave1_start_session", "wave1_append_event", "wave1_preview_aggregate", "wave1_purge_session"]);
  assert.doesNotMatch(JSON.stringify(calls.map(({ args }) => args)), /fixtures\/|ownership-map|fixture validate|assertion failed|18742|4396|prompt|path|diff|raw_tokens/i);
});

test("host DTO validators reject raw content and retain only allowlisted categorical interactions", () => {
  assert.deepEqual(validateHostPreflight(preflight), preflight); assert.deepEqual(validateHostSession(session), session); assert.deepEqual(validateHostAcknowledgement({ accepted: true, sequence: 1, timestampMs: 2 }), { accepted: true, sequence: 1, timestampMs: 2 }); assert.equal(isParticipantId("P01"), true); assert.equal(isParticipantId("P00"), false);
  assert.deepEqual(validateInteraction({ taskId: "proofline-1", interaction: "cited_evidence_opened" }), { taskId: "proofline-1", interaction: "cited_evidence_opened" });
  assert.throws(() => validateInteraction({ taskId: "proofline-1", interaction: "cited_evidence_opened", prompt: "fixture text" }), /allowlisted/i);
  assert.throws(() => validateInteraction({ taskId: "proofline-4", interaction: "approval_decided", approvalDecision: "approve" }), /requires approved/i);
  assert.equal(validateAggregate(aggregate).eventCount, 8); assert.throws(() => validateAggregate({ ...aggregate, tokens: 18742 }), /aggregate-only/i);
});

test("measurement reducer receives host values but never manufactures namespace, IDs, or time", () => {
  let state = reduceWave1MeasurementState(initialWave1MeasurementState, { type: "preflight", capture: preflight, fixture: preflight, retention: null });
  state = reduceWave1MeasurementState(state, { type: "session", capture: session.preflight, fixture: session.preflight, retention: null, sessionNamespace: session.namespace });
  state = reduceWave1MeasurementState(state, { type: "ack", eventType: "proofline-1:activity_rendered", acknowledgement: { accepted: true } });
  assert.equal(state.sessionNamespace, session.namespace); assert.deepEqual(state.acknowledgements["proofline-1:activity_rendered"], { accepted: true }); assert.equal(Object.hasOwn(state, "sessionId"), false); assert.doesNotMatch(JSON.stringify(state), /timestampMs|startedAtMs|opaque-host-value/);
});
