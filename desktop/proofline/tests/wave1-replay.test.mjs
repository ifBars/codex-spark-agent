import assert from "node:assert/strict";
import test from "node:test";
import { createWave1Adapter, isParticipantId, validateAggregate, validateEvent, validateHostAcknowledgement, validateHostPreflight, validateHostSession, validatePurge } from "../src/wave1-bridge.js";
import { initialWave1MeasurementState, reduceWave1MeasurementState } from "../src/wave1-ledger.js";
import { getWave1Scenario, initialWave1ReplayViewState, reduceWave1ReplayViewState, wave1FixtureRequest, wave1TaskGroup } from "../src/wave1-replay.js";

const fixtureRequest = wave1FixtureRequest();
const fixture = { ...fixtureRequest, verified: true, build_verified: true };
const retention = {
  status: "encrypted_local",
  purge_status: "ready",
  retention_deadline_days: 30,
  retention_deadline_status: "active",
};
const preflight = {
  capture_mode: "host_authoritative",
  countable: true,
  fixture,
  retention: { ...retention, retention_deadline_status: "not_started" },
  build: { git_sha: "a".repeat(40), dirty: false },
};
const session = {
  capture_mode: "host_authoritative",
  countable: true,
  participant_id: "P01",
  session_namespace: "wave1-12345678",
  fixture,
  retention,
};
const aggregate = {
  schema: "spark.proofline.validation.aggregate.v1",
  event_count: 8,
  invalid_preflight_attempt_count: 0,
  task_counts: [{ task_id: "proofline-1", count: 3 }],
  outcome_counts: [{ outcome: "success", count: 3 }],
  hint_count: 1,
  abandonment_count: 1,
  first_activity_ms: 67,
  retention,
  download_ready: true,
};
const purge = {
  purged: true,
  next_session_namespace: "wave1-87654321",
  retention: {
    status: "not_persisted",
    purge_status: "crypto_erased",
    retention_deadline_days: 30,
    retention_deadline_status: "not_started",
  },
};

test("all five rich rehearsal task states remain accessible", () => {
  assert.equal(wave1TaskGroup.tasks.length, 5);
  const repo = getWave1Scenario("repo-brief");
  const change = getWave1Scenario("completed-change");
  const failed = getWave1Scenario("failed-validation");
  const approval = getWave1Scenario("pending-approval");
  const usage = getWave1Scenario("partial-usage");
  assert.match(repo.citation.source, /:\d+$/);
  assert.equal(change.files.length, 2);
  assert.equal(change.validation, "Passed fixture validation");
  assert.match(failed.failureCommand, /^fixture validate/);
  assert.equal(failed.choices.length, 3);
  assert.match(approval.policy, /Approve fixture-only/i);
  assert.deepEqual(usage.sourceReportedTokens, { input: 18742, output: 4396 });
  assert.equal(usage.pricingState, "unavailable");
});

test("visible replay state remains display-only and creates no telemetry identity", () => {
  let state = initialWave1ReplayViewState;
  for (const action of [{ type: "start" }, { type: "open-evidence" }, { type: "open-change" }, { type: "recover", choice: "Restore simulated checkpoint" }, { type: "decide", decision: "approve" }, { type: "usage" }]) state = reduceWave1ReplayViewState(state, action);
  assert.equal(state.openedRepoEvidence, true);
  assert.equal(state.openedDiff, true);
  assert.equal(state.recoveryChoice, "Restore simulated checkpoint");
  assert.equal(state.approvalDecision, "approve");
  assert.equal(state.usageViewed, true);
  assert.doesNotMatch(JSON.stringify(state), /event_id|timestamp|occurred_at|sequence|session/i);
});

test("browser adapter remains visibly non-countable with a zero aggregate and no-op purge", async () => {
  const adapter = createWave1Adapter({ hostAvailable: false });
  assert.equal(adapter.kind, "browser");
  assert.equal((await adapter.preflight(fixtureRequest)).countable, false);
  assert.equal(
    (
      await adapter.startSession({
        participantId: "P01",
        fixture: fixtureRequest,
      })
    ).countable,
    false,
  );
  assert.equal((await adapter.appendEvent({ event_type: "task_outcome" })).acknowledged, false);
  assert.deepEqual(await adapter.previewAggregate(), {
    schema: "spark.proofline.validation.aggregate.v1",
    event_count: 0,
    invalid_preflight_attempt_count: 0,
    task_counts: [],
    outcome_counts: [],
    hint_count: 0,
    abandonment_count: 0,
    first_activity_ms: null,
    retention: {
      status: "not_persisted",
      purge_status: "not_applicable",
      retention_deadline_days: 0,
      retention_deadline_status: "not_applicable",
    },
    download_ready: false,
  });
  assert.deepEqual(await adapter.purgeSession(), {
    purged: false,
    next_session_namespace: "browser-rehearsal",
    retention: {
      status: "not_persisted",
      purge_status: "not_applicable",
      retention_deadline_days: 0,
      retention_deadline_status: "not_applicable",
    },
  });
});

test("adapter sends only the constrained snake_case renderer DTO", async () => {
  const calls = [];
  const invoke = async (command, args) => {
    calls.push({ command, args });
    if (command === "wave1_preflight") return preflight;
    if (command === "wave1_start_session") return session;
    if (command === "wave1_append_event") {
      return { acknowledged: true, event_type: args.event.event_type };
    }
    if (command === "wave1_preview_aggregate") return aggregate;
    return purge;
  };
  const adapter = createWave1Adapter({ invoke });
  await adapter.preflight(fixtureRequest);
  await adapter.startSession({ participantId: "P01", fixture: fixtureRequest });
  await adapter.appendEvent({
    event_type: "run_submitted",
    participant_id: "P01",
    task_id: "proofline-1",
    outcome: "success",
    capture_mode: "host_authoritative",
  });
  await adapter.previewAggregate({ download: true });
  await adapter.purgeSession();
  assert.deepEqual(calls, [
    { command: "wave1_preflight", args: { fixture: fixtureRequest } },
    {
      command: "wave1_start_session",
      args: { participant_id: "P01", fixture: fixtureRequest },
    },
    {
      command: "wave1_append_event",
      args: {
        event: {
          event_type: "run_submitted",
          participant_id: "P01",
          task_id: "proofline-1",
          outcome: "success",
          capture_mode: "host_authoritative",
        },
      },
    },
    { command: "wave1_preview_aggregate", args: { download: true } },
    { command: "wave1_purge_session", args: { confirm: true } },
  ]);
  const serialized = JSON.stringify(calls);
  assert.doesNotMatch(serialized, /thread_id|parent_thread_id|session_id|event_id|occurred_at|timestamp|sequence|content|text/);
});

test("host validators accept exact reports and reject unallowlisted response fields", () => {
  assert.deepEqual(validateHostPreflight(preflight), preflight);
  const blockedPreflight = {
    capture_mode: "host_authoritative",
    countable: false,
    fixture: {
      id: "unavailable",
      revision: "unavailable",
      sha256: "unavailable",
      verified: false,
      build_verified: false,
    },
    retention: {
      ...retention,
      status: "not_persisted",
      retention_deadline_status: "not_started",
    },
    build: { git_sha: "unknown", dirty: false },
    reason: "native lifecycle boundary is not verified; countability remains fail-closed",
  };
  assert.deepEqual(validateHostPreflight(blockedPreflight), blockedPreflight);
  assert.deepEqual(validateHostSession(session, "P01"), session);
  assert.deepEqual(
    validateHostAcknowledgement({
      acknowledged: true,
      event_type: "activity_rendered",
      latency_ms: 67,
    }),
    { acknowledged: true, event_type: "activity_rendered", latency_ms: 67 },
  );
  assert.deepEqual(validateAggregate(aggregate), aggregate);
  assert.deepEqual(validatePurge(purge), purge);
  assert.throws(() => validateHostSession({ ...session, thread_id: "native-only" }, "P01"), /not verified/i);
  assert.throws(
    () =>
      validateHostAcknowledgement({
        acknowledged: true,
        event_type: "run_submitted",
        event_id: "native-only",
      }),
    /invalid/i,
  );
  assert.throws(() => validateAggregate({ ...aggregate, thread_id: "native-only" }), /aggregate-only/i);
  assert.throws(() => {
    const { invalid_preflight_attempt_count: _, ...missingDenominator } = aggregate;
    validateAggregate(missingDenominator);
  }, /aggregate-only/i);
  assert.throws(
    () =>
      validateAggregate({
        ...aggregate,
        task_counts: [{ task_id: "proofline-6", count: 1 }],
      }),
    /aggregate-only/i,
  );
  assert.throws(
    () =>
      validateAggregate({
        ...aggregate,
        retention: { status: "encrypted_local", purge_status: "ready" },
      }),
    /aggregate-only/i,
  );
});

test("renderer events reject identity, time, sequence, and freeform fields", () => {
  const event = {
    event_type: "task_outcome",
    participant_id: "P01",
    task_id: "proofline-3",
    outcome: "hinted",
    capture_mode: "host_authoritative",
  };
  assert.equal(isParticipantId("P01"), true);
  assert.equal(isParticipantId("P00"), false);
  assert.deepEqual(validateEvent(event), event);
  assert.throws(() => validateEvent({ ...event, event_type: "recovery_selected" }), /allowlisted/i);
  for (const extra of [{ prompt: "fixture evidence" }, { thread_id: "renderer-owned" }, { event_id: "renderer-owned" }, { occurred_at: "2026-08-01T00:00:00Z" }, { timestamp_ms: 1 }, { sequence: 1 }, { content: "freeform" }]) assert.throws(() => validateEvent({ ...event, ...extra }), /allowlisted/i);
});

test("measurement reducer receives validated host reports without inventing authority", () => {
  let state = reduceWave1MeasurementState(initialWave1MeasurementState, {
    type: "preflight",
    capture: preflight,
    fixture,
    retention: preflight.retention,
  });
  state = reduceWave1MeasurementState(state, {
    type: "session",
    capture: session,
    fixture,
    retention,
    sessionNamespace: session.session_namespace,
  });
  state = reduceWave1MeasurementState(state, {
    type: "ack",
    eventType: "activity_rendered:success",
    acknowledgement: {
      acknowledged: true,
      event_type: "activity_rendered",
      latency_ms: 67,
    },
  });
  assert.equal(state.sessionNamespace, session.session_namespace);
  assert.equal(state.acknowledgements["activity_rendered:success"].latency_ms, 67);
  const afterPurge = reduceWave1MeasurementState(state, {
    type: "purged",
    capture: { ...preflight, retention: purge.retention },
    retention: purge.retention,
    nextSessionNamespace: purge.next_session_namespace,
  });
  assert.equal(afterPurge.sessionNamespace, null);
  assert.equal(afterPurge.purged, true);
});
