import { invoke as tauriInvoke, isTauri } from "@tauri-apps/api/core";

export const LIFECYCLE_COMMANDS = Object.freeze({
  begin: "proofline_lifecycle_begin",
  uiReady: "proofline_lifecycle_ui_ready",
  runSubmitted: "proofline_lifecycle_run_submitted",
  firstVisible: "proofline_lifecycle_first_visible",
});

function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function hasOnlyKeys(value, keys) {
  return isPlainObject(value) && Object.keys(value).length === keys.length && keys.every((key) => Object.hasOwn(value, key));
}

function isOpaque(value) {
  return typeof value === "string" && value.length > 0 && value.length <= 256;
}

function receipt(value, identifier, acknowledgement) {
  if (!hasOnlyKeys(value, [identifier, "challenge", "ack"]) || !isOpaque(value[identifier]) || !isOpaque(value.challenge) || value.ack !== acknowledgement) {
    throw new Error("Proofline lifecycle receipt must contain only its host-issued identifier, challenge, and acknowledgement");
  }
  return value;
}

export function validateLaunchChallenge(value) {
  if (!hasOnlyKeys(value, ["launch_id", "challenge"]) || !isOpaque(value.launch_id) || !isOpaque(value.challenge)) {
    throw new Error("Proofline launch challenge is invalid");
  }
  return value;
}

export function validateRunChallenge(value) {
  if (!hasOnlyKeys(value, ["run_id", "challenge"]) || !isOpaque(value.run_id) || !isOpaque(value.challenge)) {
    throw new Error("Proofline run challenge is invalid");
  }
  return value;
}

export function validateUiReadyReceipt(value) {
  return receipt(value, "launch_id", "ui_ready");
}

export function validateFirstVisibleReceipt(value) {
  return receipt(value, "run_id", "first_visible");
}

export function createLifecycleAdapter({ invoke, hostAvailable = isTauri() } = {}) {
  const host = typeof invoke === "function" || hostAvailable === true;
  const call = invoke ?? tauriInvoke;
  return {
    kind: host ? "tauri" : "browser",
    async beginLaunch() {
      return host ? validateLaunchChallenge(await call(LIFECYCLE_COMMANDS.begin)) : null;
    },
    async acknowledgeUiReady(value) {
      if (!host) return { acknowledged: false };
      return call(LIFECYCLE_COMMANDS.uiReady, { receipt: validateUiReadyReceipt(value) });
    },
    async beginRun() {
      return host ? validateRunChallenge(await call(LIFECYCLE_COMMANDS.runSubmitted)) : null;
    },
    async acknowledgeFirstVisible(value) {
      if (!host) return { acknowledged: false };
      return call(LIFECYCLE_COMMANDS.firstVisible, { receipt: validateFirstVisibleReceipt(value) });
    },
  };
}

function defaultScheduleFrame(callback) {
  if (typeof globalThis.requestAnimationFrame === "function") return globalThis.requestAnimationFrame(callback);
  return globalThis.setTimeout(callback, 0);
}

function defaultInteractive(node) {
  return Boolean(node?.isConnected) && node.disabled !== true && node.getAttribute?.("aria-disabled") !== "true";
}

/**
 * Maintains renderer-local delivery state only. The host owns identity, ordering,
 * and all timing. This controller never takes a renderer clock reading.
 */
export function createLifecycleReceiptController({ adapter, scheduleFrame = defaultScheduleFrame, isInteractive = defaultInteractive } = {}) {
  if (!adapter) throw new Error("Proofline lifecycle adapter is required");
  let launch = null;
  let run = null;
  let uiReadyState = "idle";
  let firstVisibleState = "idle";

  function installLaunch(nextLaunch) {
    const changed = !launch || launch.launch_id !== nextLaunch.launch_id || launch.challenge !== nextLaunch.challenge;
    launch = nextLaunch;
    if (changed) uiReadyState = "idle";
    return launch;
  }

  function installRun(nextRun) {
    const changed = !run || run.run_id !== nextRun.run_id || run.challenge !== nextRun.challenge;
    run = nextRun;
    if (changed) firstVisibleState = "idle";
    return run;
  }

  return {
    get launch() {
      return launch;
    },
    get run() {
      return run;
    },
    async beginLaunch() {
      const nextLaunch = await adapter.beginLaunch();
      return nextLaunch ? installLaunch(nextLaunch) : null;
    },
    async acknowledgeUiReadyWhenInteractive({ taskRail, composer }) {
      if (!launch || uiReadyState !== "idle" || !isInteractive(taskRail) || !isInteractive(composer)) return false;
      uiReadyState = "pending";
      try {
        await adapter.acknowledgeUiReady({ launch_id: launch.launch_id, challenge: launch.challenge, ack: "ui_ready" });
        uiReadyState = "sent";
        return true;
      } catch (error) {
        uiReadyState = "idle";
        throw error;
      }
    },
    async beginRun() {
      const nextRun = await adapter.beginRun();
      return nextRun ? installRun(nextRun) : null;
    },
    acknowledgeFirstVisibleAfterFrames({ isVisible }) {
      if (!run || firstVisibleState !== "idle" || typeof isVisible !== "function" || !isVisible()) return false;
      const expectedRun = run;
      firstVisibleState = "pending";
      scheduleFrame(() => {
        if (run !== expectedRun || !isVisible()) {
          firstVisibleState = "idle";
          return;
        }
        scheduleFrame(async () => {
          if (run !== expectedRun || !isVisible()) {
            firstVisibleState = "idle";
            return;
          }
          try {
            await adapter.acknowledgeFirstVisible({ run_id: expectedRun.run_id, challenge: expectedRun.challenge, ack: "first_visible" });
            if (run === expectedRun) firstVisibleState = "sent";
          } catch {
            if (run === expectedRun) firstVisibleState = "idle";
          }
        });
      });
      return true;
    },
  };
}

/**
 * A run becomes a renderer-visible submitted state only after the native host
 * has accepted it and issued the opaque challenge. Keeping this small ordering
 * boundary outside React makes the receipt protocol deterministic to test.
 */
export async function commitSubmittedRunAfterHostAck({ controller, commit }) {
  if (!controller || typeof commit !== "function") throw new Error("Proofline run commit requires a lifecycle controller and commit callback");
  const run = await controller.beginRun();
  if (!run) return false;
  commit(run);
  return true;
}
