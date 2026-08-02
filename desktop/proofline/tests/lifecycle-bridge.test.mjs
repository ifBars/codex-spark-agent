import assert from "node:assert/strict";
import test from "node:test";
import {
  LIFECYCLE_COMMANDS,
  commitSubmittedRunAfterHostAck,
  createLifecycleAdapter,
  createLifecycleReceiptController,
  validateFirstVisibleReceipt,
  validateLaunchChallenge,
  validateRunChallenge,
  validateUiReadyReceipt,
} from "../src/lifecycle-bridge.js";

test("lifecycle adapter uses exact challenge-bound Tauri DTOs without renderer timing", async () => {
  const calls = [];
  const adapter = createLifecycleAdapter({
    invoke: async (command, args) => {
      calls.push({ command, args });
      if (command === LIFECYCLE_COMMANDS.begin) return { launch_id: "launch-a", challenge: "challenge-a" };
      if (command === LIFECYCLE_COMMANDS.runSubmitted) return { run_id: "run-a", challenge: "challenge-b" };
      return { accepted: true };
    },
  });

  assert.deepEqual(await adapter.beginLaunch(), { launch_id: "launch-a", challenge: "challenge-a" });
  await adapter.acknowledgeUiReady({ launch_id: "launch-a", challenge: "challenge-a", ack: "ui_ready" });
  assert.deepEqual(await adapter.beginRun(), { run_id: "run-a", challenge: "challenge-b" });
  await adapter.acknowledgeFirstVisible({ run_id: "run-a", challenge: "challenge-b", ack: "first_visible" });

  assert.deepEqual(calls, [
    { command: LIFECYCLE_COMMANDS.begin, args: undefined },
    { command: LIFECYCLE_COMMANDS.uiReady, args: { receipt: { launch_id: "launch-a", challenge: "challenge-a", ack: "ui_ready" } } },
    { command: LIFECYCLE_COMMANDS.runSubmitted, args: undefined },
    { command: LIFECYCLE_COMMANDS.firstVisible, args: { receipt: { run_id: "run-a", challenge: "challenge-b", ack: "first_visible" } } },
  ]);
  assert.doesNotMatch(JSON.stringify(calls), /timestamp|duration|sequence|first_paint|elapsed/i);
});

test("lifecycle DTOs reject unknown identifiers, timestamps, durations, and sequence fields", () => {
  assert.deepEqual(validateLaunchChallenge({ launch_id: "launch-a", challenge: "challenge-a" }), { launch_id: "launch-a", challenge: "challenge-a" });
  assert.deepEqual(validateRunChallenge({ run_id: "run-a", challenge: "challenge-b" }), { run_id: "run-a", challenge: "challenge-b" });
  assert.deepEqual(validateUiReadyReceipt({ launch_id: "launch-a", challenge: "challenge-a", ack: "ui_ready" }), { launch_id: "launch-a", challenge: "challenge-a", ack: "ui_ready" });
  assert.deepEqual(validateFirstVisibleReceipt({ run_id: "run-a", challenge: "challenge-b", ack: "first_visible" }), { run_id: "run-a", challenge: "challenge-b", ack: "first_visible" });
  for (const extra of [{ timestamp_ms: 1 }, { duration_ms: 1 }, { sequence: 1 }, { event_id: "renderer" }, { run_id: "other" }]) {
    assert.throws(() => validateUiReadyReceipt({ launch_id: "launch-a", challenge: "challenge-a", ack: "ui_ready", ...extra }), /only/i);
  }
  assert.throws(() => validateFirstVisibleReceipt({ run_id: "run-a", challenge: "challenge-b", ack: "first_visible", launch_id: "launch-a" }), /only/i);
});

test("browser rehearsal never requests lifecycle challenges or emits receipts", async () => {
  const adapter = createLifecycleAdapter({ hostAvailable: false });
  const controller = createLifecycleReceiptController({ adapter });

  assert.equal(await controller.beginLaunch(), null);
  assert.equal(await controller.acknowledgeUiReadyWhenInteractive({ taskRail: {}, composer: {} }), false);
  assert.equal(await controller.beginRun(), null);
  assert.equal(controller.acknowledgeFirstVisibleAfterFrames({ isVisible: () => true }), false);
});

test("ui readiness requires both interactive anchors, sends once, and resets only for a new challenge", async () => {
  const receipts = [];
  const launches = [
    { launch_id: "launch-a", challenge: "challenge-a" },
    { launch_id: "launch-a", challenge: "challenge-a" },
    { launch_id: "launch-b", challenge: "challenge-b" },
  ];
  const adapter = {
    beginLaunch: async () => launches.shift(),
    acknowledgeUiReady: async (value) => receipts.push(value),
    beginRun: async () => null,
    acknowledgeFirstVisible: async () => {},
  };
  const controller = createLifecycleReceiptController({ adapter, isInteractive: (node) => node?.ready === true });

  await controller.beginLaunch();
  assert.equal(await controller.acknowledgeUiReadyWhenInteractive({ taskRail: { ready: true }, composer: { ready: false } }), false);
  assert.equal(await controller.acknowledgeUiReadyWhenInteractive({ taskRail: { ready: true }, composer: { ready: true } }), true);
  assert.equal(await controller.acknowledgeUiReadyWhenInteractive({ taskRail: { ready: true }, composer: { ready: true } }), false);
  await controller.beginLaunch();
  assert.equal(await controller.acknowledgeUiReadyWhenInteractive({ taskRail: { ready: true }, composer: { ready: true } }), false);
  await controller.beginLaunch();
  assert.equal(await controller.acknowledgeUiReadyWhenInteractive({ taskRail: { ready: true }, composer: { ready: true } }), true);
  assert.deepEqual(receipts, [
    { launch_id: "launch-a", challenge: "challenge-a", ack: "ui_ready" },
    { launch_id: "launch-b", challenge: "challenge-b", ack: "ui_ready" },
  ]);
});

test("first-visible receipt waits for two rendered frames and does not claim compositor paint", async () => {
  const frames = [];
  const receipts = [];
  const controller = createLifecycleReceiptController({
    adapter: {
      beginLaunch: async () => null,
      acknowledgeUiReady: async () => {},
      beginRun: async () => ({ run_id: "run-a", challenge: "challenge-a" }),
      acknowledgeFirstVisible: async (value) => receipts.push(value),
    },
    scheduleFrame: (callback) => frames.push(callback),
  });
  let visible = true;

  await controller.beginRun();
  assert.equal(controller.acknowledgeFirstVisibleAfterFrames({ isVisible: () => visible }), true);
  assert.equal(frames.length, 1);
  frames.shift()();
  assert.equal(frames.length, 1);
  frames.shift()();
  await Promise.resolve();
  assert.deepEqual(receipts, [{ run_id: "run-a", challenge: "challenge-a", ack: "first_visible" }]);
  assert.equal(controller.acknowledgeFirstVisibleAfterFrames({ isVisible: () => visible }), false);

  visible = false;
  await controller.beginRun();
  assert.equal(controller.acknowledgeFirstVisibleAfterFrames({ isVisible: () => visible }), false);
  assert.deepEqual(receipts, [{ run_id: "run-a", challenge: "challenge-a", ack: "first_visible" }]);
});

test("host run acknowledgement precedes the submitted state and both visible frames", async () => {
  let resolveRun;
  const phases = [];
  const frames = [];
  let submitted = false;
  const controller = createLifecycleReceiptController({
    adapter: {
      beginLaunch: async () => null,
      acknowledgeUiReady: async () => {},
      beginRun: () => {
        phases.push("host_run_requested");
        return new Promise((resolve) => {
          resolveRun = resolve;
        });
      },
      acknowledgeFirstVisible: async (value) => phases.push(`first_visible:${value.run_id}`),
    },
    scheduleFrame: (callback) => frames.push(callback),
  });

  const accepted = commitSubmittedRunAfterHostAck({
    controller,
    commit: () => {
      submitted = true;
      phases.push("submitted_state_committed");
    },
  });
  await Promise.resolve();
  assert.deepEqual(phases, ["host_run_requested"]);
  assert.equal(submitted, false);
  assert.equal(controller.acknowledgeFirstVisibleAfterFrames({ isVisible: () => submitted }), false);

  resolveRun({ run_id: "run-after-host-ack", challenge: "challenge-after-host-ack" });
  assert.equal(await accepted, true);
  assert.deepEqual(phases, ["host_run_requested", "submitted_state_committed"]);
  assert.equal(submitted, true);
  assert.equal(controller.acknowledgeFirstVisibleAfterFrames({ isVisible: () => submitted }), true);
  frames.shift()();
  frames.shift()();
  await Promise.resolve();
  assert.deepEqual(phases, ["host_run_requested", "submitted_state_committed", "first_visible:run-after-host-ack"]);
});
