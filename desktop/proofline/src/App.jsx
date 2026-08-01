import { useEffect, useMemo, useReducer, useState } from "react";
import { ArrowRight, CaretDown, CheckCircle, Circle, Clock, Cube, Gear, GitBranch, Info, Minus, PaperPlaneTilt, PencilSimple, ShieldCheck, Sparkle, Tag, UserCircle, X } from "@phosphor-icons/react";
import { authorityModes, getTaskFixture, initialProoflineViewState, prototypeSubmissionNotice, reduceProoflineViewState, taskGroups } from "./proofline-state.js";
import { createWave1Adapter, isParticipantId } from "./wave1-bridge.js";
import { initialWave1MeasurementState, reduceWave1MeasurementState } from "./wave1-ledger.js";
import { getWave1Scenario, wave1FixtureRequest } from "./wave1-replay.js";

function ThreadItem({ thread, active, onClick }) {
  return <button className={`thread-item ${active ? "selected" : ""}`} onClick={onClick} type="button"><span className="thread-copy"><span className="thread-title">{thread.title}</span><span className="thread-meta">{thread.time}<span className="dot">•</span>{thread.elapsed}</span></span>{thread.complete ? <CheckCircle size={17} /> : <Circle size={17} />}</button>;
}

function CaptureState({ measurement }) {
  const countable = measurement.capture?.countable === true;
  const verified = measurement.fixture?.verified === true && measurement.fixture?.build_verified === true;
  return <section className={`measurement-state ${countable ? "verified" : "rehearsal"}`} aria-live="polite">
    <div><span>Capture mode</span><strong>{countable ? "Host-authoritative" : "Browser rehearsal"}</strong></div>
    <div><span>Fixture / build</span><strong>{verified ? "Verified" : "Not host-verified"}</strong></div>
    <div><span>Counting</span><strong>{countable ? "Countable" : "Non-countable"}</strong></div>
    <div><span>Retention</span><strong>{measurement.retention?.status ?? "Awaiting preflight"}</strong></div>
  </section>;
}

function AggregatePanel({ aggregate }) {
  if (!aggregate) return null;
  return <div className="replay-export" role="status"><strong>Aggregate-only preview</strong><span>{aggregate.event_count} categorical events. Hints: {aggregate.hint_count}; abandonments: {aggregate.abandonment_count}; first activity: {aggregate.first_activity_ms} ms. No prompts, paths, commands, diffs, raw token values, IDs, timestamps, or network payloads are shown.</span></div>;
}

function Wave1Measurement({ selected, measurement, dispatch, setNotice }) {
  const adapter = useMemo(() => createWave1Adapter(), []);
  const scenario = getWave1Scenario(selected.scenario);
  const active = measurement.phase === "active" && measurement.capture?.countable === true;

  useEffect(() => {
    let alive = true;
    adapter.preflight(wave1FixtureRequest()).then((result) => {
      if (!alive) return;
      dispatch({ type: "preflight", capture: result, fixture: result.fixture, retention: result.retention });
      setNotice(result.countable ? "Host preflight passed; enter a pseudonymous participant ID to start." : "Browser rehearsal is functional but deliberately non-countable; host verification is required to capture measurement.");
    }).catch((error) => alive && dispatch({ type: "error", error: error.message }));
    return () => { alive = false; };
  }, [adapter, dispatch, setNotice]);

  async function startSession() {
    if (!isParticipantId(measurement.participantId)) { dispatch({ type: "error", error: "Use a pseudonymous participant ID from P01 through P99." }); return; }
    try {
      const result = await adapter.startSession({ participantId: measurement.participantId, fixture: wave1FixtureRequest() });
      dispatch({ type: "session", capture: result, fixture: result.fixture, retention: result.retention, sessionNamespace: result.session_namespace });
      if (!result.countable) { setNotice("Browser rehearsal started. It does not create a countable session or persist measurement."); return; }
      for (const eventType of ["app_ready", "run_submitted", "activity_rendered"]) {
        const acknowledgement = await adapter.appendEvent({ event_type: eventType, participant_id: measurement.participantId, task_id: scenario.taskId, outcome: "success", capture_mode: "host_authoritative" });
        dispatch({ type: "ack", eventType, acknowledgement });
      }
      setNotice("Host acknowledged app-ready and first-activity timing. Measurement is active.");
    } catch (error) { dispatch({ type: "error", error: error.message }); }
  }

  async function recordOutcome(outcome) {
    if (!active) { setNotice("Outcome controls remain rehearsal-only until host preflight and a countable session succeed."); return; }
    try {
      const acknowledgement = await adapter.appendEvent({ event_type: "task_outcome", participant_id: measurement.participantId, task_id: scenario.taskId, outcome, capture_mode: "host_authoritative" });
      dispatch({ type: "ack", eventType: `task_outcome:${scenario.taskId}:${outcome}`, acknowledgement });
      setNotice(`Host acknowledged the categorical ${outcome} outcome.`);
    } catch (error) { dispatch({ type: "error", error: error.message }); }
  }

  async function preview(download = false) {
    try {
      const aggregate = await adapter.previewAggregate({ download });
      if (!measurement.capture?.countable) { setNotice("Browser rehearsal has no aggregate download. Connect the host to create a countable aggregate."); return; }
      dispatch({ type: "aggregate", aggregate });
      setNotice(download && aggregate.download_ready ? "Host prepared an aggregate-only download." : "Host returned an aggregate-only preview.");
    } catch (error) { dispatch({ type: "error", error: error.message }); }
  }

  async function purge() {
    try {
      const result = await adapter.purgeSession();
      dispatch({ type: "purged", retention: result.retention, nextSessionNamespace: result.next_session_namespace ?? result.session_namespace });
      setNotice(result.countable === false ? "Browser rehearsal has nothing persisted to purge." : "Host confirmed purge; a fresh host-supplied namespace is required before the next session.");
    } catch (error) { dispatch({ type: "error", error: error.message }); }
  }

  return <>
    <section className="replay-fixture-controls" aria-label="Wave 1 measurement controls">
      <div><span>Wave 1 measurement</span><strong>Host-authoritative when available</strong><small>Fixture identity is sent to the host for verification; the renderer never verifies an archive itself.</small></div>
      <p>This surface accepts only a pseudonymous participant ID and fixed categorical outcomes. It never sends free-text telemetry.</p>
      <CaptureState measurement={measurement} />
      <div className="measurement-start"><label>Pseudonymous participant ID<input aria-label="Pseudonymous participant ID" value={measurement.participantId} onChange={(event) => dispatch({ type: "participant", participantId: event.target.value.toUpperCase() })} inputMode="text" maxLength="3" placeholder="P01" pattern="P(?:0[1-9]|[1-9][0-9])" /></label><button type="button" className="action-button primary" onClick={startSession} disabled={measurement.phase === "active" && measurement.capture?.countable}><span>{active ? "Measurement active" : "Start measurement"}</span><ArrowRight size={17} /></button></div>
      <div className="replay-actions"><button type="button" className="action-button" onClick={() => preview(false)}><span>Preview aggregate</span><ArrowRight size={17} /></button><button type="button" className="action-button" onClick={() => preview(true)}><span>Download aggregate</span><ArrowRight size={17} /></button><button type="button" className="action-button" onClick={purge}><span>Purge session</span><X size={17} /></button></div>
      {measurement.sessionNamespace && <p className="replay-event-note">{measurement.capture?.countable ? `Host session namespace: ${measurement.sessionNamespace}. It is host-supplied and replaced after a host purge.` : "Browser rehearsal state is volatile and non-countable; browser reload retains no measurement state."}</p>}
      {measurement.purged && <p className="replay-event-note">Retention status: {measurement.retention?.purge_status ?? "purged"}.</p>}
      {measurement.error && <p className="measurement-error" role="alert">{measurement.error}</p>}
      <AggregatePanel aggregate={measurement.aggregate} />
    </section>

    <section className="replay-section" aria-label="Wave 1 categorical outcome controls">
      <div className="section-heading"><h2>{scenario.title ?? selected.title}</h2><span className="fixture-label">Categorical only</span></div>
      <div className="replay-detail"><p>{selected.summary}</p><p>{selected.scenario === "partial-usage" ? "Usage is partial and pricing unavailable. Raw token values are not captured by this measurement surface." : "Fixture evidence remains a rehearsal aid; host acknowledgement is the only countable lifecycle evidence."}</p></div>
      <div className="replay-actions"><button type="button" className="action-button primary" onClick={() => recordOutcome("success")}><span>Complete task</span><CheckCircle size={17} /></button><button type="button" className="action-button" onClick={() => recordOutcome("failure")}><span>Record failure</span><ArrowRight size={17} /></button><button type="button" className="action-button" onClick={() => recordOutcome("hinted")}><span>Request hint</span><Info size={17} /></button><button type="button" className="action-button" onClick={() => recordOutcome("abandoned")}><span>Abandon task</span><X size={17} /></button></div>
      <p className="replay-event-note">Outcome categories preserve denominators for completed, failed, hinted, and abandoned tasks. No explanation text is collected.</p>
    </section>
  </>;
}

export function App() {
  const [viewState, setViewState] = useState(initialProoflineViewState);
  const [measurement, dispatchMeasurement] = useReducer(reduceWave1MeasurementState, initialWave1MeasurementState);
  const [composer, setComposer] = useState(""); const [notice, setNotice] = useState(""); const [model, setModel] = useState("GPT-5.3-Codex-Spark"); const [reasoning, setReasoning] = useState("Medium"); const [authorityMode, setAuthorityMode] = useState("ask");
  const selected = getTaskFixture(viewState.selectedId); const isWave1 = Boolean(selected.scenario); const authority = authorityModes[authorityMode];
  const dispatchView = (event) => setViewState((state) => reduceProoflineViewState(state, event));
  function submitComposer(event) { event.preventDefault(); if (!composer.trim()) return; setNotice(prototypeSubmissionNotice()); setComposer(""); }
  return <main className="app-shell"><header className="window-bar"><div className="window-product"><span className="mini-mark"><Sparkle size={15} weight="fill" /></span><span>Proofline for Spark</span></div><div className="window-controls" aria-label="Window controls"><button type="button" aria-label="Minimize"><Minus size={16} /></button><button type="button" aria-label="Maximize"><Cube size={14} /></button><button type="button" aria-label="Close"><X size={16} /></button></div></header>
    <div className="workspace-shell"><aside className="thread-rail" aria-label="Spark task history"><div className="rail-brand"><Sparkle className="brand-mark" size={42} weight="regular" /><div><strong>Proofline</strong><span>for Spark</span></div><button type="button" className="icon-button new-task" aria-label="New task" onClick={() => setNotice("A fresh task is ready for your next instruction.")}><PencilSimple size={19} /></button></div><nav>{taskGroups.map((group) => <div key={group.label}><p className={`period-label ${group.label === "This week" ? "week-label" : ""}`}>{group.label}</p><div className="thread-list">{group.tasks.map((thread) => <ThreadItem key={thread.id} thread={thread} active={viewState.selectedId === thread.id} onClick={() => { dispatchView({ type: "select-task", id: thread.id }); setNotice(""); }} />)}</div></div>)}</nav><div className="rail-footer"><button type="button" className="icon-button" aria-label="Settings"><Gear size={18} /></button><button type="button" className="icon-button" aria-label="Profile"><UserCircle size={19} /></button></div></aside>
      <section className="task-surface"><div className="task-content"><div className={`outcome-meta ${selected.complete ? "" : "in-progress"}`}><CheckCircle size={19} weight="regular" /><strong>{selected.complete ? "COMPLETED" : "IN PROGRESS"}</strong><span className="dot">•</span><span>{selected.time}</span><span className="dot">•</span><span>{selected.elapsed}</span></div><h1>{selected.title}</h1><p className="summary">{selected.summary}</p><p className="prototype-boundary" role="note"><Info size={15} weight="fill" />{isWave1 ? "Browser mode is rehearsal-only. Countable Wave 1 evidence requires host verification and acknowledgements." : "Simulated prototype data — not connected to a Spark session, repository, or provider account."}</p>
        {isWave1 ? <Wave1Measurement selected={selected} measurement={measurement} dispatch={dispatchMeasurement} setNotice={setNotice} /> : <section className="evidence-section evidence-unavailable"><div className="section-heading"><h2>Evidence</h2><span className="evidence-state">Not recorded</span></div><div className="evidence-empty"><Info size={18} /><div><strong>No detailed fixture evidence</strong><span>This task is deliberately unavailable until a fixture or host record exists.</span></div></div></section>}
        {notice && <div className="action-notice" role="status"><CheckCircle size={16} />{notice}</div>}
        <form className="composer" onSubmit={submitComposer}><textarea value={composer} onChange={(event) => setComposer(event.target.value)} placeholder="What should Spark do next?" aria-label="What should Spark do next?" /><p className="authority-note"><ShieldCheck size={15} />{authority.note}</p><div className="composer-footer"><label>Model<select value={model} onChange={(event) => setModel(event.target.value)}><option>GPT-5.3-Codex-Spark</option><option>GPT-5.6-Luna</option></select><CaretDown size={14} /></label><label>Reasoning<select value={reasoning} onChange={(event) => setReasoning(event.target.value)}><option>Low</option><option>Medium</option><option>High</option></select><CaretDown size={14} /></label><label>Mode<select value={authorityMode} onChange={(event) => setAuthorityMode(event.target.value)}>{Object.entries(authorityModes).map(([value, option]) => <option value={value} key={value}>{option.label}</option>)}</select><CaretDown size={14} /></label><button className="send-button" type="submit" aria-label="Stage prototype instruction"><PaperPlaneTilt size={20} weight="bold" /></button></div></form>
      </div></section></div>
    <footer className="status-ribbon"><span><GitBranch size={17} />{isWave1 ? (measurement.capture?.countable ? "Host measurement" : "Browser rehearsal") : "Fixture branch main"}</span><i /><span>{isWave1 ? (measurement.fixture?.verified ? "Fixture / build verified" : "Host verification required") : "Checkpoint unavailable"}</span><i /><span><Clock size={17} />Elapsed {selected.elapsed}</span><i /><span><Cube size={17} />{isWave1 ? "No raw usage captured" : "Tokens unavailable"}</span><i /><span><Tag size={17} />Pricing unavailable</span><span className="status-spacer" /><span className="local"><b />Local-first</span><span><ShieldCheck size={17} />Capture shown</span></footer>
  </main>;
}
