import { invoke as tauriInvoke, isTauri } from "@tauri-apps/api/core";

const EVENT_TYPES = new Set(["run_submitted", "activity_rendered", "task_outcome"]);
const OUTCOMES = new Set(["success", "failure", "hinted", "abandoned"]);
const OUTBOUND_FORBIDDEN_KEYS = new Set(["prompt", "path", "paths", "command", "commands", "diff", "raw_tokens", "tokens", "content", "text", "thread_id", "parent_thread_id", "session_id", "event_id", "occurred_at", "timestamp", "timestamp_ms", "sequence"]);

export const WAVE1_COMMANDS = Object.freeze({
  preflight: "wave1_preflight",
  startSession: "wave1_start_session",
  appendEvent: "wave1_append_event",
  previewAggregate: "wave1_preview_aggregate",
  purgeSession: "wave1_purge_session",
});

function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function hasOnlyKeys(value, required, optional = []) {
  if (!isPlainObject(value)) return false;
  const keys = Object.keys(value);
  const allowed = new Set([...required, ...optional]);
  return required.every((key) => Object.hasOwn(value, key)) && keys.every((key) => allowed.has(key));
}

function containsForbiddenOutboundKey(value) {
  if (!value || typeof value !== "object") return false;
  return Object.entries(value).some(([key, nested]) => OUTBOUND_FORBIDDEN_KEYS.has(key) || containsForbiddenOutboundKey(nested));
}

function isNonNegativeInteger(value) {
  return Number.isSafeInteger(value) && value >= 0;
}

function isSafeNamespace(value) {
  return typeof value === "string" && /^wave1-[a-z0-9-]{8,}$/i.test(value);
}

function validFixture(value, requireVerified = false) {
  return hasOnlyKeys(value, ["id", "revision", "sha256", "verified", "build_verified"]) && typeof value.id === "string" && value.id.length > 0 && typeof value.revision === "string" && value.revision.length > 0 && (/^[a-f0-9]{64}$/i.test(value.sha256) || (!requireVerified && value.sha256 === "unavailable")) && typeof value.verified === "boolean" && typeof value.build_verified === "boolean" && (!requireVerified || (value.verified && value.build_verified));
}

function validRetention(value) {
  return hasOnlyKeys(value, ["status", "purge_status", "retention_deadline_days", "retention_deadline_status"]) && ["encrypted_local", "not_persisted"].includes(value.status) && ["ready", "crypto_erased", "not_applicable"].includes(value.purge_status) && isNonNegativeInteger(value.retention_deadline_days) && ["not_started", "active", "expired", "not_applicable"].includes(value.retention_deadline_status);
}

function validBuild(value) {
  return hasOnlyKeys(value, ["git_sha", "dirty"]) && (/^[a-f0-9]{40}$/i.test(value.git_sha) || value.git_sha === "unknown") && typeof value.dirty === "boolean";
}

function validOptionalReason(value) {
  return value === undefined || (typeof value === "string" && value.length <= 500);
}

export function isParticipantId(value) {
  return typeof value === "string" && /^P(?:0[1-9]|[1-9][0-9])$/.test(value);
}

export function validateHostPreflight(value) {
  if (!hasOnlyKeys(value, ["capture_mode", "countable", "fixture", "retention", "build"], ["reason"]) || value.capture_mode !== "host_authoritative" || typeof value.countable !== "boolean" || !validFixture(value.fixture) || !validRetention(value.retention) || !validBuild(value.build) || (value.countable && (!/^[a-f0-9]{40}$/i.test(value.build.git_sha) || value.build.dirty || !validFixture(value.fixture, true))) || !validOptionalReason(value.reason)) {
    throw new Error("Wave 1 host preflight is invalid");
  }
  return value;
}

export function validateHostSession(value, participantId) {
  if (!hasOnlyKeys(value, ["capture_mode", "countable", "participant_id", "session_namespace", "fixture", "retention"]) || value.capture_mode !== "host_authoritative" || value.countable !== true || value.participant_id !== participantId || !isSafeNamespace(value.session_namespace) || !validFixture(value.fixture, true) || !validRetention(value.retention)) {
    throw new Error("Wave 1 host session is not verified");
  }
  return value;
}

export function validateEvent(event) {
  const allowed = ["event_type", "participant_id", "task_id", "outcome", "capture_mode"];
  if (!hasOnlyKeys(event, allowed) || containsForbiddenOutboundKey(event) || !EVENT_TYPES.has(event.event_type) || !isParticipantId(event.participant_id) || !/^proofline-[1-5]$/.test(event.task_id) || !OUTCOMES.has(event.outcome) || event.capture_mode !== "host_authoritative") {
    throw new Error("Wave 1 event is not an allowlisted categorical DTO");
  }
  return event;
}

export function validateHostAcknowledgement(value) {
  if (!hasOnlyKeys(value, ["acknowledged", "event_type"]) || value.acknowledged !== true || !EVENT_TYPES.has(value.event_type)) {
    throw new Error("Wave 1 host acknowledgement is invalid");
  }
  return value;
}

function validTaskCount(value) {
  return hasOnlyKeys(value, ["task_id", "count"]) && /^proofline-[1-5]$/.test(value.task_id) && isNonNegativeInteger(value.count);
}

function validOutcomeCount(value) {
  return hasOnlyKeys(value, ["outcome", "count"]) && OUTCOMES.has(value.outcome) && isNonNegativeInteger(value.count);
}

export function validateAggregate(value) {
  const allowed = ["schema", "event_count", "invalid_preflight_attempt_count", "task_counts", "outcome_counts", "hint_count", "abandonment_count", "first_activity_ms", "retention", "download_ready"];
  if (!hasOnlyKeys(value, allowed) || value.schema !== "spark.proofline.validation.aggregate.v1" || !isNonNegativeInteger(value.event_count) || !isNonNegativeInteger(value.invalid_preflight_attempt_count) || !Array.isArray(value.task_counts) || !value.task_counts.every(validTaskCount) || !Array.isArray(value.outcome_counts) || !value.outcome_counts.every(validOutcomeCount) || !isNonNegativeInteger(value.hint_count) || !isNonNegativeInteger(value.abandonment_count) || value.first_activity_ms !== null || !validRetention(value.retention) || typeof value.download_ready !== "boolean") {
    throw new Error("Wave 1 aggregate is not aggregate-only");
  }
  return value;
}

export function validatePurge(value) {
  if (!hasOnlyKeys(value, ["purged", "next_session_namespace", "retention"]) || value.purged !== true || !isSafeNamespace(value.next_session_namespace) || !validRetention(value.retention)) {
    throw new Error("Wave 1 purge acknowledgement is invalid");
  }
  return value;
}

function browserRetention() {
  return {
    status: "not_persisted",
    purge_status: "not_applicable",
    retention_deadline_days: 0,
    retention_deadline_status: "not_applicable",
  };
}

function browserPreflight() {
  return {
    capture_mode: "browser_rehearsal",
    countable: false,
    fixture: {
      id: "",
      revision: "",
      sha256: "",
      verified: false,
      build_verified: false,
    },
    retention: browserRetention(),
    build: { git_sha: "browser", dirty: true },
  };
}

function browserAggregate() {
  return {
    schema: "spark.proofline.validation.aggregate.v1",
    event_count: 0,
    invalid_preflight_attempt_count: 0,
    task_counts: [],
    outcome_counts: [],
    hint_count: 0,
    abandonment_count: 0,
    first_activity_ms: null,
    retention: browserRetention(),
    download_ready: false,
  };
}

export function createWave1Adapter({ invoke, hostAvailable = isTauri() } = {}) {
  const host = typeof invoke === "function" || hostAvailable === true;
  const call = invoke ?? tauriInvoke;
  return {
    kind: host ? "tauri" : "browser",
    async preflight(fixture) {
      return host ? validateHostPreflight(await call(WAVE1_COMMANDS.preflight, { fixture })) : browserPreflight();
    },
    async startSession({ participantId, fixture }) {
      if (!isParticipantId(participantId)) throw new Error("Participant ID must be P01 through P99");
      return host
        ? validateHostSession(
            await call(WAVE1_COMMANDS.startSession, {
              participant_id: participantId,
              fixture,
            }),
            participantId,
          )
        : {
            ...browserPreflight(),
            participant_id: participantId,
            session_namespace: "browser-rehearsal",
          };
    },
    async appendEvent(event) {
      return host
        ? validateHostAcknowledgement(
            await call(WAVE1_COMMANDS.appendEvent, {
              event: validateEvent(event),
            }),
          )
        : { acknowledged: false, event_type: event.event_type };
    },
    async previewAggregate({ download = false } = {}) {
      return host ? validateAggregate(await call(WAVE1_COMMANDS.previewAggregate, { download })) : browserAggregate();
    },
    async purgeSession() {
      return host
        ? validatePurge(await call(WAVE1_COMMANDS.purgeSession, { confirm: true }))
        : {
            purged: false,
            next_session_namespace: "browser-rehearsal",
            retention: browserRetention(),
          };
    },
  };
}
