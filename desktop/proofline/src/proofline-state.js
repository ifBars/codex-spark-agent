const fixtureFiles = [
  ["fixture/usage/history.rs", "+182", "-10", "Simulated fork-aware history and lineage work"],
  ["fixture/usage/record.rs", "+64", "-6", "Simulated fork lineage fields on usage events"],
  ["fixture/db/schema.rs", "+38", "-2", "Simulated history index update"],
  ["fixture/migrations/add_fork_id.sql", "+26", "-0", "Simulated fork column migration"],
  ["fixture/api/history.rs", "+41", "-3", "Simulated fork context in history responses"],
  ["fixture/tests/usage_history_fork.rs", "+96", "-1", "Simulated lineage attribution checks"],
];

const fixtureValidations = [
  ["cargo fmt --all -- --check", "2s"],
  ["cargo clippy --workspace --all-targets -- -D warnings", "6s"],
  ["cargo test --workspace --all-features", "19s"],
  ["cargo test --test usage_history_fork", "3s"],
];

const unavailableEvidence = {
  kind: "unavailable",
  message: "No detailed fixture evidence is recorded for this prototype task.",
  files: [],
  validations: [],
  checkpoint: null,
  tokens: null,
  work: [],
};

const simulatedForkEvidence = {
  kind: "simulated",
  message: "Simulated fixture evidence — not connected to a Spark session, repository, or provider account.",
  files: fixtureFiles,
  validations: fixtureValidations,
  checkpoint: "23f7c9a",
  tokens: "18,742 in · 4,396 out",
  work: [
    ["Plan", "Resolve fixture lineage before aggregating source-reported token observations."],
    ["Evidence", "Use simulated trace and replay boundaries; no local files were read."],
    ["Validation", "Show recorded fixture checks only; this prototype did not execute them."],
  ],
};

export const taskGroups = [
  {
    label: "Today",
    tasks: [
      { id: "fork", title: "Add fork-aware usage history", summary: "Spark added fork-aware usage history so usage tracking correctly attributes source-reported tokens to the originating fork lineage. History queries now include fork context; pricing remains unavailable.", time: "10:42 AM", elapsed: "31s", complete: true, evidence: simulatedForkEvidence },
      { id: "errors", title: "Improve error messaging", summary: "Spark clarified recoverable command failures with direct next steps and preserved the original diagnostic context.", time: "9:58 AM", elapsed: "18s", complete: true, evidence: unavailableEvidence },
      { id: "sessions", title: "Refactor session store", summary: "Spark separated session metadata from transcript persistence while maintaining checkpoint compatibility.", time: "9:22 AM", elapsed: "27s", complete: true, evidence: unavailableEvidence },
      { id: "quota", title: "Tighten quota validation", summary: "Spark is checking quota boundaries before a provider request is created.", time: "8:47 AM", elapsed: "21s", complete: false, evidence: unavailableEvidence },
    ],
  },
  {
    label: "This week",
    tasks: [
      { id: "json", title: "Add export to JSON", summary: "Spark prepared a portable local evidence export with explicit schema and provenance fields.", time: "Yesterday", elapsed: "35s", complete: true, evidence: unavailableEvidence },
      { id: "indexes", title: "Optimize index queries", summary: "Spark reduced redundant history lookups while preserving the current result ordering.", time: "Yesterday", elapsed: "42s", complete: true, evidence: unavailableEvidence },
      { id: "retention", title: "Introduce retention job", summary: "Spark is defining a local retention policy and a visible purge boundary before implementation.", time: "Mon", elapsed: "29s", complete: false, evidence: unavailableEvidence },
      { id: "integration", title: "Add integration tests", summary: "Spark added coverage for session lineage, validation evidence, and unavailable pricing states.", time: "Mon", elapsed: "52s", complete: true, evidence: unavailableEvidence },
      { id: "admin", title: "Scaffold admin commands", summary: "Spark is separating privileged maintenance commands from the default interactive surface.", time: "Mon", elapsed: "16s", complete: false, evidence: unavailableEvidence },
    ],
  },
];

export const authorityModes = {
  ask: {
    label: "Ask (read-only tools)",
    note: "Ask exposes read-only native tools. It is not a privacy or sandbox guarantee.",
  },
  work: {
    label: "Work (OS-user access)",
    note: "Work can use native tools with your OS-user access. It is not sandboxed or privately confined.",
  },
};

export const initialProoflineViewState = {
  selectedId: "fork",
  showWork: false,
  showFiles: false,
  reviewing: false,
  focusedFile: null,
};

export function getTaskFixture(id) {
  return taskGroups.flatMap((group) => group.tasks).find((task) => task.id === id) ?? taskGroups[0].tasks[0];
}

export function hasDetailedEvidence(task) {
  return task.evidence.kind === "simulated";
}

export function prototypeSubmissionNotice() {
  return "Prototype instruction staged — no request was sent.";
}

export function reduceProoflineViewState(state, event) {
  const selected = getTaskFixture(state.selectedId);

  switch (event.type) {
    case "select-task":
      return { ...initialProoflineViewState, selectedId: getTaskFixture(event.id).id };
    case "toggle-review":
      return hasDetailedEvidence(selected) ? { ...state, reviewing: !state.reviewing } : state;
    case "toggle-files":
      return hasDetailedEvidence(selected)
        ? { ...state, showFiles: !state.showFiles, focusedFile: state.showFiles ? null : state.focusedFile }
        : state;
    case "select-file":
      return hasDetailedEvidence(selected) && selected.evidence.files.some(([path]) => path === event.path)
        ? { ...state, showFiles: true, focusedFile: event.path }
        : state;
    case "toggle-work":
      return hasDetailedEvidence(selected) ? { ...state, showWork: !state.showWork } : state;
    default:
      return state;
  }
}
