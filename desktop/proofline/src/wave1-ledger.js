export const initialWave1MeasurementState = Object.freeze({ phase: "preflight", participantId: "", capture: null, fixture: null, retention: null, sessionNamespace: null, acknowledgements: {}, aggregate: null, purged: false, error: null });
export function reduceWave1MeasurementState(state, action) {
  switch (action.type) {
    case "participant": return { ...state, participantId: action.participantId, error: null };
    case "preflight": return { ...state, phase: "preflight", capture: action.capture, fixture: action.fixture, retention: action.retention, error: null };
    case "session": return { ...state, phase: action.capture.countable ? "active" : "rehearsal", capture: action.capture, fixture: action.fixture ?? state.fixture, retention: action.retention, sessionNamespace: action.sessionNamespace, purged: false, error: null };
    case "ack": return { ...state, acknowledgements: { ...state.acknowledgements, [action.eventType]: action.acknowledgement }, error: null };
    case "aggregate": return { ...state, aggregate: action.aggregate, error: null };
    case "purged": return { ...state, phase: "preflight", capture: action.capture, fixture: action.capture?.fixture ?? state.fixture, retention: action.retention, sessionNamespace: null, acknowledgements: {}, aggregate: null, purged: true, error: null };
    case "error": return { ...state, error: action.error };
    default: return state;
  }
}
