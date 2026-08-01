import { invoke as tauriInvoke, isTauri } from "@tauri-apps/api/core";

const INTERACTIONS = new Set(["replay_submitted", "activity_rendered", "cited_evidence_opened", "change_inspected", "validation_inspected", "recovery_selected", "approval_presented", "approval_decided", "usage_boundary_viewed", "aggregate_previewed"]);
const FORBIDDEN_KEYS = new Set(["prompt", "path", "paths", "command", "commands", "diff", "raw_tokens", "tokens", "content", "text", "participant_id", "session_id", "event_id", "occurred_at"]);

export const WAVE1_COMMANDS = Object.freeze({ preflight: "wave1_preflight", startSession: "wave1_start_session", appendEvent: "wave1_append_event", previewAggregate: "wave1_preview_aggregate", purgeSession: "wave1_purge_session" });
export function isParticipantId(value) { return typeof value === "string" && /^P(?:0[1-9]|[1-9][0-9])$/.test(value); }
function hasForbiddenKey(value) { return Boolean(value && typeof value === "object" && Object.entries(value).some(([key, nested]) => FORBIDDEN_KEYS.has(key) || hasForbiddenKey(nested))); }
function isSafeNamespace(value) { return typeof value === "string" && /^wave1-[a-z0-9-]{8,}$/i.test(value); }
function isPreflight(value) { return value && typeof value === "object" && typeof value.countable === "boolean" && typeof value.fixtureManifestSha256 === "string" && typeof value.evidenceVerified === "boolean" && value.build && typeof value.build.gitSha === "string" && typeof value.build.dirty === "boolean"; }

export function validateHostPreflight(value) {
  if (!isPreflight(value) || hasForbiddenKey(value)) throw new Error("Wave 1 host preflight contained forbidden telemetry fields");
  return value;
}
export function validateHostSession(value) {
  if (!value || typeof value !== "object" || hasForbiddenKey(value) || !isSafeNamespace(value.namespace) || !Number.isInteger(value.nextSequence) || !isPreflight(value.preflight)) throw new Error("Wave 1 host session is not verified");
  return value;
}
export function validateInteraction(interaction) {
  const allowed = new Set(["taskId", "interaction", "approvalDecision"]);
  if (!interaction || typeof interaction !== "object" || hasForbiddenKey(interaction) || Object.keys(interaction).some((key) => !allowed.has(key)) || !/^proofline-[1-5]$/.test(interaction.taskId) || !INTERACTIONS.has(interaction.interaction)) throw new Error("Wave 1 interaction is not an allowlisted category");
  if (interaction.interaction === "approval_decided") {
    if (!matchesDecision(interaction.approvalDecision)) throw new Error("approval_decided requires approved or denied");
  } else if (interaction.approvalDecision != null) throw new Error("approvalDecision is only permitted for approval_decided");
  return interaction;
}
function matchesDecision(value) { return value === "approved" || value === "denied"; }
export function validateHostAcknowledgement(value) {
  if (!value || typeof value !== "object" || hasForbiddenKey(value) || typeof value.accepted !== "boolean" || !Number.isInteger(value.sequence) || !Number.isFinite(value.timestampMs)) throw new Error("Wave 1 host acknowledgement is invalid");
  return value;
}
export function validateAggregate(value) {
  const allowed = new Set(["schema", "countable", "eventCount", "eventCountsByTask", "eventCountsByCategory", "sessionNamespaceRotated"]);
  if (!value || typeof value !== "object" || hasForbiddenKey(value) || Object.keys(value).some((key) => !allowed.has(key)) || typeof value.countable !== "boolean" || !Number.isInteger(value.eventCount) || !Array.isArray(value.eventCountsByTask) || !Array.isArray(value.eventCountsByCategory)) throw new Error("Wave 1 aggregate is not aggregate-only");
  return value;
}
function browserResponse() { return { countable: false, reason: "Browser rehearsal is non-countable", fixtureManifestSha256: "", evidenceVerified: false, build: { gitSha: "browser", dirty: true } }; }

export function createWave1Adapter({ invoke, hostAvailable = isTauri() } = {}) {
  const host = typeof invoke === "function" || hostAvailable === true;
  const call = invoke ?? tauriInvoke;
  return {
    kind: host ? "tauri" : "browser",
    async preflight() { return host ? validateHostPreflight(await call(WAVE1_COMMANDS.preflight)) : browserResponse(); },
    async startSession() { return host ? validateHostSession(await call(WAVE1_COMMANDS.startSession)) : { namespace: "browser-rehearsal", nextSequence: 0, preflight: browserResponse() }; },
    async appendInteraction(interaction) { return host ? validateHostAcknowledgement(await call(WAVE1_COMMANDS.appendEvent, { interaction: validateInteraction(interaction) })) : { accepted: false, sequence: 0, timestampMs: 0 }; },
    async previewAggregate() { return host ? validateAggregate(await call(WAVE1_COMMANDS.previewAggregate)) : { schema: "spark.proofline.validation.v1", countable: false, eventCount: 0, eventCountsByTask: [], eventCountsByCategory: [], sessionNamespaceRotated: false }; },
    async purgeSession() { return host ? validateHostPreflight(await call(WAVE1_COMMANDS.purgeSession, { confirm: true })) : browserResponse(); },
  };
}
