import assert from "node:assert/strict";
import test from "node:test";
import { createWave1Adapter, isParticipantId, validateAggregate, validateAppendEvent, validateHostAcknowledgement, validateHostPreflight, validateHostSession } from "../src/wave1-bridge.js";
import { initialWave1MeasurementState, reduceWave1MeasurementState } from "../src/wave1-ledger.js";
import { getWave1Scenario, wave1FixtureRequest, wave1TaskGroup } from "../src/wave1-replay.js";

const fixture = { ...wave1FixtureRequest(), verified: true, build_verified: true };
const retention = { status: "encrypted_local", purge_status: "available" };
const preflight = { capture_mode: "host_authoritative", countable: true, fixture, retention };
const session = { capture_mode: "host_authoritative", countable: true, participant_id: "P01", session_namespace: "wave1_6a8f4d2c", retention };

test("Wave 1 has five fixed rehearsal tasks without logging evidence payloads", () => {
  assert.equal(wave1TaskGroup.tasks.length, 5);
  assert.deepEqual(wave1TaskGroup.tasks.map((task) => task.scenario), ["repo-brief", "completed-change", "failed-validation", "pending-approval", "partial-usage"]);
  assert.equal(getWave1Scenario("partial-usage").usageState, "partial");
  assert.equal(getWave1Scenario("partial-usage").pricingState, "unavailable");
  assert.equal(isParticipantId("P01"), true);
  assert.equal(isParticipantId("P99"), true);
  assert.equal(isParticipantId("P00"), false);
  assert.equal(isParticipantId("participant-1"), false);
});

test("browser adapter fails closed as a non-countable rehearsal and never invokes a host", async () => {
  const adapter = createWave1Adapter({ invoke: undefined });
  const rehearsal = await adapter.preflight(wave1FixtureRequest());
  assert.equal(adapter.kind, "browser");
  assert.equal(rehearsal.countable, false);
  assert.equal(rehearsal.capture_mode, "browser_rehearsal");
  assert.equal((await adapter.startSession({ participantId: "P01", fixture: wave1FixtureRequest() })).countable, false);
  assert.equal((await adapter.appendEvent({ event_type: "task_outcome" })).acknowledged, false);
  assert.equal((await adapter.previewAggregate()).aggregate, null);
});

test("host DTO validation accepts categorical acknowledgements and rejects forbidden telemetry", () => {
  assert.deepEqual(validateHostPreflight(preflight), preflight);
  assert.deepEqual(validateHostSession(session, "P01"), session);
  assert.deepEqual(validateHostAcknowledgement({ acknowledged: true, event_type: "app_ready", latency_ms: 31 }), { acknowledged: true, event_type: "app_ready", latency_ms: 31 });
  assert.deepEqual(validateHostAcknowledgement({ acknowledged: true, event_type: "activity_rendered", latency_ms: 112 }), { acknowledged: true, event_type: "activity_rendered", latency_ms: 112 });
  assert.throws(() => validateHostPreflight({ ...preflight, prompt: "do not store" }), /forbidden/i);
  assert.throws(() => validateHostSession({ ...session, session_namespace: "short" }, "P01"), /not a verified/i);
  assert.throws(() => validateHostAcknowledgement({ acknowledged: true, event_type: "activity_rendered", latency_ms: -1 }), /lacks host timing/i);
  assert.throws(() => validateAppendEvent({ event_type: "task_outcome", participant_id: "P01", task_id: "proofline-1", outcome: "success", capture_mode: "host_authoritative", command: "cargo test" }), /categorical/i);
});

test("host adapter calls only the five Wave 1 commands and sends no raw content", async () => {
  const calls = [];
  const invoke = async (command, args) => {
    calls.push({ command, args });
    if (command === "wave1_preflight") return preflight;
    if (command === "wave1_start_session") return session;
    if (command === "wave1_append_event") return { acknowledged: true, event_type: args.event.event_type, latency_ms: args.event.event_type === "app_ready" ? 12 : args.event.event_type === "activity_rendered" ? 83 : undefined };
    if (command === "wave1_preview_aggregate") return { schema: "spark.proofline.aggregate.v1", event_count: 5, task_counts: { "proofline-1": 1 }, outcome_counts: { success: 1, failure: 1, hinted: 1, abandoned: 1 }, hint_count: 1, abandonment_count: 1, first_activity_ms: 83, retention, download_ready: Boolean(args.download) };
    return { purged: true, next_session_namespace: "wave1_749c513b", retention };
  };
  const adapter = createWave1Adapter({ invoke });
  await adapter.preflight(wave1FixtureRequest()); await adapter.startSession({ participantId: "P01", fixture: wave1FixtureRequest() });
  await adapter.appendEvent({ event_type: "task_outcome", participant_id: "P01", task_id: "proofline-1", outcome: "hinted", capture_mode: "host_authoritative" });
  const aggregate = await adapter.previewAggregate({ download: true }); await adapter.purgeSession();
  assert.equal(aggregate.hint_count, 1); assert.equal(aggregate.abandonment_count, 1);
  assert.deepEqual(calls.map((call) => call.command), ["wave1_preflight", "wave1_start_session", "wave1_append_event", "wave1_preview_aggregate", "wave1_purge_session"]);
  for (const { args } of calls) assert.doesNotMatch(JSON.stringify(args), /prompt|path|diff|raw_tokens|occurred_at|event_id/i);
});

test("aggregate preview holds denominators but rejects identity and raw telemetry", () => {
  const aggregate = { schema: "spark.proofline.aggregate.v1", event_count: 8, task_counts: { "proofline-1": 3 }, outcome_counts: { success: 2, failure: 2, hinted: 2, abandoned: 2 }, hint_count: 2, abandonment_count: 2, first_activity_ms: 70, retention, download_ready: true };
  assert.equal(validateAggregate(aggregate).event_count, 8);
  assert.throws(() => validateAggregate({ ...aggregate, participant_id: "P01" }), /aggregate-only/i);
  assert.throws(() => validateAggregate({ ...aggregate, tokens: 18800 }), /aggregate-only|forbidden/i);
});

test("measurement reducer consumes host values and never creates IDs, timestamps, or namespaces", () => {
  let state = initialWave1MeasurementState;
  state = reduceWave1MeasurementState(state, { type: "participant", participantId: "P01" });
  state = reduceWave1MeasurementState(state, { type: "preflight", capture: preflight, fixture, retention });
  state = reduceWave1MeasurementState(state, { type: "session", capture: session, fixture, retention, sessionNamespace: session.session_namespace });
  state = reduceWave1MeasurementState(state, { type: "ack", eventType: "activity_rendered", acknowledgement: { acknowledged: true, event_type: "activity_rendered", latency_ms: 83 } });
  assert.equal(state.sessionNamespace, "wave1_6a8f4d2c");
  assert.equal(state.acknowledgements.activity_rendered.latency_ms, 83);
  assert.equal(Object.prototype.hasOwnProperty.call(state, "event_id"), false);
  assert.equal(Object.prototype.hasOwnProperty.call(state, "occurred_at"), false);
  assert.equal(Object.prototype.hasOwnProperty.call(state, "timestamp"), false);
  const purged = reduceWave1MeasurementState(state, { type: "purged", retention: { status: "encrypted_local", purge_status: "purged" }, nextSessionNamespace: "wave1_749c513b" });
  assert.equal(purged.phase, "preflight"); assert.equal(purged.sessionNamespace, "wave1_749c513b"); assert.deepEqual(purged.acknowledgements, {});
});
