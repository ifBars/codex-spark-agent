import { invoke as tauriInvoke, isTauri } from "@tauri-apps/api/core";

const EVENT_TYPES = new Set(["app_ready", "run_submitted", "activity_rendered", "task_outcome"]);
const OUTCOMES = new Set(["success", "failure", "hinted", "abandoned"]);
const FORBIDDEN_KEYS = new Set(["prompt", "path", "paths", "command", "commands", "diff", "raw_tokens", "tokens", "content", "text", "session_id", "event_id", "occurred_at"]);
export const WAVE1_COMMANDS = Object.freeze({ preflight: "wave1_preflight", startSession: "wave1_start_session", appendEvent: "wave1_append_event", previewAggregate: "wave1_preview_aggregate", purgeSession: "wave1_purge_session" });
export function isParticipantId(value) { return typeof value === "string" && /^P(?:0[1-9]|[1-9][0-9])$/.test(value); }
function hasForbiddenKey(value) { return Boolean(value && typeof value === "object" && Object.entries(value).some(([key, nested]) => FORBIDDEN_KEYS.has(key) || hasForbiddenKey(nested))); }
function isSafeNamespace(value) { return typeof value === "string" && /^wave1-[a-z0-9-]{8,}$/i.test(value); }
function validFixture(value) { return value && typeof value === "object" && typeof value.id === "string" && typeof value.revision === "string" && typeof value.sha256 === "string" && typeof value.verified === "boolean" && typeof value.build_verified === "boolean"; }
function validRetention(value) { return value && typeof value === "object" && typeof value.status === "string" && typeof value.purge_status === "string"; }

export function validateHostPreflight(value) {
  if (!value || typeof value !== "object" || hasForbiddenKey(value) || !["host_authoritative", "browser_rehearsal"].includes(value.capture_mode) || typeof value.countable !== "boolean" || !validFixture(value.fixture) || !validRetention(value.retention) || !value.build || typeof value.build.git_sha !== "string" || typeof value.build.dirty !== "boolean") throw new Error("Wave 1 host preflight is invalid");
  return value;
}
export function validateHostSession(value, participantId) {
  if (!value || typeof value !== "object" || hasForbiddenKey(value) || value.capture_mode !== "host_authoritative" || value.countable !== true || value.participant_id !== participantId || !isSafeNamespace(value.session_namespace) || !validFixture(value.fixture) || !validRetention(value.retention)) throw new Error("Wave 1 host session is not verified");
  return value;
}
export function validateEvent(event) {
  const allowed = new Set(["event_type", "participant_id", "task_id", "outcome", "capture_mode"]);
  if (!event || typeof event !== "object" || hasForbiddenKey(event) || Object.keys(event).some((key) => !allowed.has(key)) || !EVENT_TYPES.has(event.event_type) || !isParticipantId(event.participant_id) || !/^proofline-[1-5]$/.test(event.task_id) || !OUTCOMES.has(event.outcome) || event.capture_mode !== "host_authoritative") throw new Error("Wave 1 event is not an allowlisted categorical DTO");
  return event;
}
export function validateHostAcknowledgement(value) {
  if (!value || typeof value !== "object" || hasForbiddenKey(value) || value.acknowledged !== true || !EVENT_TYPES.has(value.event_type)) throw new Error("Wave 1 host acknowledgement is invalid");
  if ((value.event_type === "app_ready" || value.event_type === "activity_rendered") && (!Number.isFinite(value.latency_ms) || value.latency_ms < 0)) throw new Error("Wave 1 lifecycle acknowledgement lacks host timing");
  return value;
}
export function validateAggregate(value) {
  const allowed = new Set(["schema", "event_count", "task_counts", "outcome_counts", "hint_count", "abandonment_count", "first_activity_ms", "retention", "download_ready"]);
  if (!value || typeof value !== "object" || hasForbiddenKey(value) || Object.keys(value).some((key) => !allowed.has(key)) || !Number.isInteger(value.event_count) || !Array.isArray(value.task_counts) || !Array.isArray(value.outcome_counts) || !Number.isInteger(value.hint_count) || !Number.isInteger(value.abandonment_count) || !validRetention(value.retention) || typeof value.download_ready !== "boolean") throw new Error("Wave 1 aggregate is not aggregate-only");
  return value;
}
export function validatePurge(value) { if (!value || typeof value !== "object" || hasForbiddenKey(value) || value.purged !== true || !isSafeNamespace(value.next_session_namespace) || !validRetention(value.retention)) throw new Error("Wave 1 purge acknowledgement is invalid"); return value; }
function browserPreflight() { return { capture_mode: "browser_rehearsal", countable: false, fixture: { id: "", revision: "", sha256: "", verified: false, build_verified: false }, retention: { status: "not_persisted", purge_status: "not_applicable" }, build: { git_sha: "browser", dirty: true } }; }

export function createWave1Adapter({ invoke, hostAvailable = isTauri() } = {}) {
  const host = typeof invoke === "function" || hostAvailable === true; const call = invoke ?? tauriInvoke;
  return { kind: host ? "tauri" : "browser", async preflight(fixture) { return host ? validateHostPreflight(await call(WAVE1_COMMANDS.preflight, { fixture })) : browserPreflight(); }, async startSession({ participantId, fixture }) { if (!isParticipantId(participantId)) throw new Error("Participant ID must be P01 through P99"); return host ? validateHostSession(await call(WAVE1_COMMANDS.startSession, { participant_id: participantId, fixture }), participantId) : { ...browserPreflight(), participant_id: participantId, session_namespace: "browser-rehearsal" }; }, async appendEvent(event) { return host ? validateHostAcknowledgement(await call(WAVE1_COMMANDS.appendEvent, { event: validateEvent(event) })) : { acknowledged: false, event_type: event.event_type }; }, async previewAggregate({ download = false } = {}) { return host ? validateAggregate(await call(WAVE1_COMMANDS.previewAggregate, { download })) : { schema: "spark.proofline.validation.aggregate.v1", event_count: 0, task_counts: [], outcome_counts: [], hint_count: 0, abandonment_count: 0, first_activity_ms: null, retention: browserPreflight().retention, download_ready: false }; }, async purgeSession() { return host ? validatePurge(await call(WAVE1_COMMANDS.purgeSession, { confirm: true })) : { purged: false, next_session_namespace: "browser-rehearsal", retention: browserPreflight().retention }; } };
}
