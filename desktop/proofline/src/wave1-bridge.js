const PARTICIPANT_ID = /^P(?:0[1-9]|[1-9][0-9])$/;
const OUTCOMES = new Set(["success", "failure", "hinted", "abandoned"]);
const EVENT_TYPES = new Set(["app_ready", "run_submitted", "activity_rendered", "task_outcome"]);
const FORBIDDEN_KEYS = new Set(["prompt", "path", "paths", "command", "commands", "diff", "raw_tokens", "tokens", "timestamp", "occurred_at", "event_id", "thread_id", "content", "text"]);

export const WAVE1_COMMANDS = Object.freeze({ preflight: "wave1_preflight", startSession: "wave1_start_session", appendEvent: "wave1_append_event", previewAggregate: "wave1_preview_aggregate", purgeSession: "wave1_purge_session" });
export function isParticipantId(value) { return typeof value === "string" && PARTICIPANT_ID.test(value); }
function hasForbiddenKey(value) { return Boolean(value && typeof value === "object" && Object.entries(value).some(([key, nested]) => FORBIDDEN_KEYS.has(key) || hasForbiddenKey(nested))); }
function isSafeNamespace(value) { return typeof value === "string" && /^[a-z0-9][a-z0-9_-]{7,127}$/i.test(value); }
function fixtureIsVerified(value) { return value && typeof value === "object" && value.verified === true && value.build_verified === true && typeof value.id === "string" && typeof value.revision === "string" && typeof value.sha256 === "string"; }
function retentionIsSafe(value) { return value && typeof value === "object" && typeof value.status === "string" && typeof value.purge_status === "string"; }

export function validateHostPreflight(value) {
  if (!value || typeof value !== "object" || hasForbiddenKey(value)) throw new Error("Wave 1 host preflight contained forbidden telemetry fields");
  if (value.capture_mode !== "host_authoritative" || value.countable !== true || !fixtureIsVerified(value.fixture) || !retentionIsSafe(value.retention)) throw new Error("Wave 1 host preflight is not countable or fixture-verified");
  return value;
}
export function validateHostSession(value, participantId) {
  if (!value || typeof value !== "object" || hasForbiddenKey(value)) throw new Error("Wave 1 host session contained forbidden telemetry fields");
  if (value.capture_mode !== "host_authoritative" || value.countable !== true || value.participant_id !== participantId || !isSafeNamespace(value.session_namespace) || !retentionIsSafe(value.retention)) throw new Error("Wave 1 host session is not a verified countable session");
  return value;
}
export function validateHostAcknowledgement(value) {
  if (!value || typeof value !== "object" || hasForbiddenKey(value) || value.acknowledged !== true || !EVENT_TYPES.has(value.event_type)) throw new Error("Wave 1 host acknowledgement is invalid");
  if ((value.event_type === "app_ready" || value.event_type === "activity_rendered") && (!Number.isFinite(value.latency_ms) || value.latency_ms < 0)) throw new Error("Wave 1 lifecycle acknowledgement lacks host timing");
  return value;
}
export function validateAggregate(value) {
  const allowed = new Set(["schema", "event_count", "task_counts", "outcome_counts", "hint_count", "abandonment_count", "first_activity_ms", "retention", "download_ready"]);
  if (!value || typeof value !== "object" || hasForbiddenKey(value) || Object.keys(value).some((key) => !allowed.has(key)) || !retentionIsSafe(value.retention) || !Number.isInteger(value.event_count)) throw new Error("Wave 1 aggregate is not aggregate-only");
  return value;
}
export function validateAppendEvent(event) {
  const allowed = new Set(["event_type", "participant_id", "task_id", "outcome", "capture_mode"]);
  if (!event || typeof event !== "object" || hasForbiddenKey(event) || !EVENT_TYPES.has(event.event_type) || !isParticipantId(event.participant_id) || typeof event.task_id !== "string" || !OUTCOMES.has(event.outcome) || event.capture_mode !== "host_authoritative" || Object.keys(event).some((key) => !allowed.has(key))) throw new Error("Wave 1 event is not a categorical host DTO");
  return event;
}
function browserResponse() { return { capture_mode: "browser_rehearsal", countable: false, fixture: { verified: false, build_verified: false }, retention: { status: "not_persisted", purge_status: "not_applicable" } }; }
export function createWave1Adapter({ invoke = globalThis?.__TAURI__?.core?.invoke } = {}) {
  const host = typeof invoke === "function";
  return {
    kind: host ? "tauri" : "browser",
    async preflight(fixture) { return host ? validateHostPreflight(await invoke(WAVE1_COMMANDS.preflight, { fixture })) : browserResponse(); },
    async startSession({ participantId, fixture }) { if (!isParticipantId(participantId)) throw new Error("Participant ID must be P01 through P99"); return host ? validateHostSession(await invoke(WAVE1_COMMANDS.startSession, { participant_id: participantId, fixture }), participantId) : { ...browserResponse(), participant_id: participantId, session_namespace: "browser-rehearsal" }; },
    async appendEvent(event) { return host ? validateHostAcknowledgement(await invoke(WAVE1_COMMANDS.appendEvent, { event: validateAppendEvent(event) })) : { acknowledged: false, capture_mode: "browser_rehearsal", countable: false, event_type: event.event_type }; },
    async previewAggregate({ download = false } = {}) { return host ? validateAggregate(await invoke(WAVE1_COMMANDS.previewAggregate, { download })) : { ...browserResponse(), aggregate: null, download_ready: false }; },
    async purgeSession() { if (!host) return { ...browserResponse(), purged: true, session_namespace: "browser-rehearsal" }; const value = await invoke(WAVE1_COMMANDS.purgeSession, {}); if (!value || typeof value !== "object" || hasForbiddenKey(value) || value.purged !== true || !isSafeNamespace(value.next_session_namespace) || !retentionIsSafe(value.retention)) throw new Error("Wave 1 purge acknowledgement is invalid"); return value; },
  };
}
export { OUTCOMES };
