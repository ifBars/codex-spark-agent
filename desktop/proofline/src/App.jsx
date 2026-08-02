import { useEffect, useMemo, useReducer, useRef, useState } from "react";
import { ArrowRight, CaretDown, CheckCircle, Circle, Clock, Cube, FileCode, Gear, GitBranch, Info, Minus, PaperPlaneTilt, PencilSimple, ShieldCheck, Sparkle, Tag, UserCircle, X } from "@phosphor-icons/react";
import { authorityModes, getTaskFixture, initialProoflineViewState, prototypeSubmissionNotice, reduceProoflineViewState, taskGroups } from "./proofline-state.js";
import { createWave1Adapter, isParticipantId } from "./wave1-bridge.js";
import { commitSubmittedRunAfterHostAck, createLifecycleAdapter, createLifecycleReceiptController } from "./lifecycle-bridge.js";
import { initialWave1MeasurementState, reduceWave1MeasurementState } from "./wave1-ledger.js";
import { getWave1Scenario, initialWave1ReplayViewState, reduceWave1ReplayViewState, wave1FixtureRequest } from "./wave1-replay.js";
import prooflineMark from "../assets/proofline-mark.png";

function ThreadItem({ thread, active, onClick }) {
  return (
    <button className={`thread-item ${active ? "selected" : ""}`} onClick={onClick} type="button">
      <span className="thread-copy">
        <span className="thread-title">{thread.title}</span>
        <span className="thread-meta">
          {thread.time}
          <span className="dot">•</span>
          {thread.elapsed}
        </span>
      </span>
      {thread.complete ? <CheckCircle size={17} /> : <Circle size={17} />}
    </button>
  );
}
function CaptureState({ measurement }) {
  const capture = measurement.capture;
  const native = capture?.capture_mode === "host_authoritative";
  const countable = capture?.countable === true;
  const verified = capture?.fixture?.verified === true && capture?.fixture?.build_verified === true && capture?.build?.dirty === false;
  const deadline = countable && measurement.retention?.retention_deadline_status === "active" ? `${measurement.retention.retention_deadline_days}-day deadline active` : "Not persisted";
  return (
    <section className={`measurement-state ${countable ? "verified" : "rehearsal"}`} aria-live="polite">
      <div>
        <span>Capture mode</span>
        <strong>{countable ? "Host-authoritative" : native ? "Native preflight blocked" : "Browser rehearsal"}</strong>
      </div>
      <div>
        <span>Fixture / build</span>
        <strong>{verified ? "Verified" : "Not host-verified"}</strong>
      </div>
      <div>
        <span>Counting</span>
        <strong>{countable ? "Countable" : "Non-countable"}</strong>
      </div>
      <div>
        <span>Retention</span>
        <strong>{deadline}</strong>
      </div>
    </section>
  );
}
function RendererReceipt({ acknowledgements }) {
  const acknowledgement = acknowledgements["activity_rendered:success"];
  if (!acknowledgement) return null;
  return (
    <div className="replay-event-note" role="status">
      Host accepted a non-authoritative renderer receipt. It is not a process-start boundary, actual first paint, or official lifecycle timing.
    </div>
  );
}
function AggregatePanel({ aggregate }) {
  if (!aggregate) return null;
  return (
    <div className="replay-export" role="status">
      <strong>Aggregate-only preview</strong>
      <span>
        {aggregate.event_count} categorical events. Hints: {aggregate.hint_count}; abandonments: {aggregate.abandonment_count}. No prompts, paths, commands, diffs, raw token values, identities, fixture content, or lifecycle records are exported.
      </span>
    </div>
  );
}
function ReplayEventNote({ countable }) {
  return <p className="replay-event-note">{countable ? "Displayed fixture content is never included in host telemetry; only supported categorical events are acknowledged." : "This browser rehearsal updates visible state only. It is non-countable and sends no telemetry."}</p>;
}

function OutcomeControls({ recordOutcome }) {
  return (
    <div className="replay-actions">
      <button type="button" className="action-button primary" onClick={() => recordOutcome("success")}>
        <span>Complete task</span>
        <CheckCircle size={17} />
      </button>
      <button type="button" className="action-button" onClick={() => recordOutcome("failure")}>
        <span>Record failure</span>
        <ArrowRight size={17} />
      </button>
      <button type="button" className="action-button" onClick={() => recordOutcome("hinted")}>
        <span>Request hint</span>
        <Info size={17} />
      </button>
      <button type="button" className="action-button" onClick={() => recordOutcome("abandoned")}>
        <span>Abandon task</span>
        <X size={17} />
      </button>
    </div>
  );
}

function RichReplay({ selected, replay, dispatchReplay, recordOutcome, countable, setNotice }) {
  const scenario = getWave1Scenario(selected.scenario);
  const display = (action, notice) => {
    dispatchReplay(action);
    setNotice(notice);
  };
  if (selected.scenario === "repo-brief")
    return (
      <section className="replay-section" aria-label="Wave 1 Task 1 replay">
        <div className="section-heading">
          <h2>Repo Brief evidence</h2>
          <span className="fixture-label">Task 1 of 5</span>
        </div>
        <div className="replay-card">
          <p>
            Likely ownership boundary: <strong>{scenario.answer}</strong>
          </p>
          <button className="action-button primary" type="button" disabled={!replay.runStarted} onClick={() => display({ type: "open-evidence" }, "Opened cited rehearsal evidence. Content remains display-only.")}>
            <span>Open cited evidence</span>
            <ArrowRight size={18} />
          </button>
        </div>
        {replay.openedRepoEvidence && (
          <div className="replay-detail">
            <strong>{scenario.citation.source}</strong>
            <p>{scenario.citation.excerpt}</p>
            <p>
              <b>Uncertainty:</b> {scenario.uncertainty}
            </p>
            <p>
              <b>Next check:</b> {scenario.nextCheck}
            </p>
          </div>
        )}
        <OutcomeControls recordOutcome={recordOutcome} />
        <ReplayEventNote countable={countable} />
      </section>
    );
  if (selected.scenario === "completed-change")
    return (
      <section className="replay-section" aria-label="Wave 1 Task 2 replay">
        <div className="section-heading">
          <h2>Completed fixture evidence</h2>
          <span className="fixture-label">Task 2 of 5</span>
        </div>
        <div className="file-ledger">
          {scenario.files.map((file) => (
            <button className="file-row" type="button" key={file} onClick={() => display({ type: "open-change" }, "Opened the simulated two-file change. No diff content was captured.")}>
              <FileCode size={15} />
              <code>{file}</code>
              <span className="added">fixture</span>
              <span className="removed">only</span>
              <span className="file-note">Simulated changed-file record</span>
            </button>
          ))}
        </div>
        <div className="replay-detail">
          <p>
            <CheckCircle size={16} /> <b>{scenario.validation}</b>
          </p>
          <p>
            Checkpoint: <strong>{scenario.checkpoint}</strong>
          </p>
          {replay.openedDiff && <p>Simulated change inspection is open. This rehearsal never reads a local repository.</p>}
        </div>
        <OutcomeControls recordOutcome={recordOutcome} />
        <ReplayEventNote countable={countable} />
      </section>
    );
  if (selected.scenario === "failed-validation")
    return (
      <section className="replay-section" aria-label="Wave 1 Task 3 replay">
        <div className="section-heading">
          <h2>Failed fixture validation</h2>
          <span className="fixture-label">Task 3 of 5</span>
        </div>
        <div className="replay-detail replay-failure">
          <p>
            <b>{scenario.validation}</b>
          </p>
          <p>{scenario.failure}</p>
          <p>
            <b>Simulated command:</b> <code>{scenario.failureCommand}</code>
          </p>
          <p>
            <b>Simulated result:</b> {scenario.failureOutput}
          </p>
          <p>This is a failed run state, not a completed result.</p>
        </div>
        <div className="replay-actions">
          {scenario.choices.map((choice) => (
            <button type="button" className="action-button" key={choice} onClick={() => display({ type: "recover", choice }, `${choice} selected in the visible rehearsal.`)}>
              <span>{choice}</span>
              <ArrowRight size={17} />
            </button>
          ))}
        </div>
        {replay.recoveryChoice && (
          <p className="replay-detail">
            <b>Selected recovery:</b> {replay.recoveryChoice}. No validation, retry, restore, or repository mutation ran.
          </p>
        )}
        <OutcomeControls recordOutcome={recordOutcome} />
        <ReplayEventNote countable={countable} />
      </section>
    );
  if (selected.scenario === "pending-approval")
    return (
      <section className="replay-section" aria-label="Wave 1 Task 4 replay">
        <div className="section-heading">
          <h2>Pending fixture approval</h2>
          <span className="fixture-label">Task 4 of 5</span>
        </div>
        <div className="replay-detail replay-approval">
          <p>
            <b>Run status: awaiting approval</b>
          </p>
          <p>Requested authority: fixture file change.</p>
          <p>
            <b>Policy card:</b> {scenario.policy}
          </p>
        </div>
        <div className="replay-actions">
          <button type="button" className="action-button" onClick={() => display({ type: "open-change" }, "Opened the fixture approval policy.")}>
            <span>Review policy</span>
            <ArrowRight size={17} />
          </button>
          <button type="button" className="action-button primary" onClick={() => display({ type: "decide", decision: "approve" }, "Approved fixture-only file change in the rehearsal.")}>
            <span>Approve fixture change</span>
            <CheckCircle size={18} />
          </button>
          <button type="button" className="action-button" onClick={() => display({ type: "decide", decision: "deny" }, "Denied fixture request in the rehearsal.")}>
            <span>Deny request</span>
            <X size={18} />
          </button>
        </div>
        {replay.approvalDecision && (
          <p className="replay-detail">
            <b>Policy outcome:</b> {replay.approvalDecision === "approve" ? "approved fixture-only file change" : "denied request"}. No authority was granted.
          </p>
        )}
        <OutcomeControls recordOutcome={recordOutcome} />
        <ReplayEventNote countable={countable} />
      </section>
    );
  return (
    <section className="replay-section" aria-label="Wave 1 Task 5 replay">
      <div className="section-heading">
        <h2>Usage and handoff boundary</h2>
        <span className="fixture-label">Task 5 of 5</span>
      </div>
      <div className="replay-detail">
        <p>
          <b>Source-reported fixture tokens:</b> {scenario.sourceReportedTokens.input.toLocaleString()} input · {scenario.sourceReportedTokens.output.toLocaleString()} output.
        </p>
        <p>
          <b>Usage coverage:</b> Partial source-reported history; this is not quota.
        </p>
        <p>
          <b>Pricing:</b> Unavailable. This does not mean free or complete.
        </p>
        <p>{scenario.handoff}</p>
      </div>
      <button type="button" className="action-button primary" onClick={() => display({ type: "usage" }, "Partial usage and unavailable pricing were reviewed.")}>
        <span>{replay.usageViewed ? "Usage boundary reviewed" : "Review usage boundary"}</span>
        <ArrowRight size={18} />
      </button>
      <OutcomeControls recordOutcome={recordOutcome} />
      <ReplayEventNote countable={countable} />
    </section>
  );
}

function Wave1Measurement({ selected, measurement, dispatch, setNotice }) {
  const adapter = useMemo(() => createWave1Adapter(), []);
  const [replay, dispatchReplay] = useReducer(reduceWave1ReplayViewState, initialWave1ReplayViewState);
  const [confirmPurge, setConfirmPurge] = useState(false);
  const active = measurement.phase === "active" && measurement.capture?.countable === true;
  const nativeBlocked = measurement.capture?.capture_mode === "host_authoritative" && measurement.capture?.countable !== true;
  useEffect(() => {
    let alive = true;
    const fixture = wave1FixtureRequest();
    adapter
      .preflight(fixture)
      .then((capture) => {
        if (!alive) return;
        dispatch({
          type: "preflight",
          capture,
          fixture: capture.fixture,
          retention: capture.retention,
        });
        setNotice(capture.countable ? "Host preflight passed. Start a host-owned Wave 1 session." : capture.capture_mode === "host_authoritative" ? "Native preflight is deliberately non-countable until lifecycle instrumentation is complete. The five tasks remain available as rehearsal." : "Browser rehearsal is functional but deliberately non-countable; host verification is required to capture measurement.");
      })
      .catch((error) => alive && dispatch({ type: "error", error: error.message }));
    return () => {
      alive = false;
    };
  }, [adapter, dispatch, setNotice]);
  async function append(eventType, outcome, allowStartingSession = false) {
    if (!active && !allowStartingSession) return;
    try {
      const acknowledgement = await adapter.appendEvent({
        event_type: eventType,
        participant_id: measurement.participantId,
        task_id: getWave1Scenario(selected.scenario).taskId,
        outcome,
        capture_mode: "host_authoritative",
      });
      dispatch({
        type: "ack",
        eventType: `${eventType}:${outcome}`,
        acknowledgement,
      });
    } catch (error) {
      dispatch({ type: "error", error: error.message });
    }
  }
  async function startSession() {
    if (!isParticipantId(measurement.participantId)) {
      dispatch({
        type: "error",
        error: "Use a pseudonymous participant label from P01 through P99.",
      });
      return;
    }
    try {
      const result = await adapter.startSession({
        participantId: measurement.participantId,
        fixture: wave1FixtureRequest(),
      });
      dispatch({
        type: "session",
        capture: result,
        fixture: result.fixture,
        retention: result.retention,
        sessionNamespace: result.session_namespace,
      });
      dispatchReplay({ type: "start" });
      if (!result.countable) {
        setNotice("Browser rehearsal started. It does not create a countable session, aggregate, or persisted measurement.");
        return;
      }
      await append("run_submitted", "success", true);
      await append("activity_rendered", "success", true);
      setNotice("Host accepted a constrained submission event and non-authoritative renderer receipt. Official lifecycle timing remains separate protocol work.");
    } catch (error) {
      dispatch({ type: "error", error: error.message });
    }
  }
  async function recordOutcome(outcome) {
    if (!active) {
      setNotice("Outcome controls remain visible in rehearsal, but only a verified host session can record them.");
      return;
    }
    await append("task_outcome", outcome);
    setNotice(`Host acknowledged the categorical ${outcome} outcome.`);
  }
  async function preview(download = false) {
    try {
      const aggregate = await adapter.previewAggregate({ download });
      if (!measurement.capture?.countable) {
        setNotice("Browser rehearsal has no countable aggregate. Connect the host to create one.");
        return;
      }
      dispatch({ type: "aggregate", aggregate });
      setNotice(download && aggregate.download_ready ? "Host prepared an aggregate-only download." : "Host returned an aggregate-only preview.");
    } catch (error) {
      dispatch({ type: "error", error: error.message });
    }
  }
  async function purge() {
    try {
      const result = await adapter.purgeSession();
      setConfirmPurge(false);
      if (!result.purged) {
        setNotice("Browser rehearsal has no persisted measurement to purge.");
        return;
      }
      dispatch({
        type: "purged",
        capture: { ...measurement.capture, retention: result.retention },
        retention: result.retention,
        nextSessionNamespace: result.next_session_namespace,
      });
      setNotice("Host confirmed purge. Start a fresh session before recording more measurement.");
    } catch (error) {
      dispatch({ type: "error", error: error.message });
    }
  }
  return (
    <>
      <section className="replay-fixture-controls" aria-label="Wave 1 measurement controls">
        <div>
          <span>Wave 1 measurement</span>
          <strong>Host-authoritative only after preflight</strong>
          <small>The renderer displays fixture evidence but sends only supported categorical event DTOs.</small>
        </div>
        <p>Enter a pseudonymous participant ID. The native host owns ledger event identity, ordering, and receipt timestamps.</p>
        <CaptureState measurement={measurement} />
        {nativeBlocked && <p className="replay-event-note">Native capture is installed but not participant-countable. Process-start and actual first-paint instrumentation remain outstanding.</p>}
        <div className="measurement-start">
          <label>
            Pseudonymous participant ID
            <input
              aria-label="Pseudonymous participant ID"
              value={measurement.participantId}
              onChange={(event) =>
                dispatch({
                  type: "participant",
                  participantId: event.target.value.toUpperCase(),
                })
              }
              maxLength="3"
              placeholder="P01"
              pattern="P(?:0[1-9]|[1-9][0-9])"
            />
          </label>
          <button type="button" className="action-button primary" onClick={startSession} disabled={active || nativeBlocked}>
            <span>{active ? "Measurement active" : nativeBlocked ? "Native preflight blocked" : "Start measurement"}</span>
            <ArrowRight size={17} />
          </button>
        </div>
        <div className="replay-actions">
          <button type="button" className="action-button" onClick={() => preview(false)}>
            <span>Preview aggregate</span>
            <ArrowRight size={17} />
          </button>
          <button type="button" className="action-button" onClick={() => preview(true)}>
            <span>Download aggregate</span>
            <ArrowRight size={17} />
          </button>
          <button type="button" className="action-button" onClick={() => setConfirmPurge(true)}>
            <span>Purge session</span>
            <X size={17} />
          </button>
        </div>
        {confirmPurge && (
          <div className="replay-detail replay-failure" role="alert">
            <p>
              <b>Purge local measurement?</b> {measurement.capture?.countable ? "This invokes host crypto-erasure for the active session." : "No countable measurement is active; confirming exercises the host or browser no-op boundary."}
            </p>
            <div className="replay-actions">
              <button type="button" className="action-button primary" onClick={purge}>
                <span>Confirm purge</span>
                <X size={17} />
              </button>
              <button type="button" className="action-button" onClick={() => setConfirmPurge(false)}>
                <span>Cancel</span>
                <ArrowRight size={17} />
              </button>
            </div>
          </div>
        )}
        {measurement.sessionNamespace && <p className="replay-event-note">{measurement.capture?.countable ? "A host-owned measurement namespace is active. Its authoritative event IDs, timestamps, and sequence stay inside the protected native ledger." : "Browser rehearsal state is volatile and non-countable; its aggregate remains zero and reload retains no measurement state."}</p>}
        <RendererReceipt acknowledgements={measurement.acknowledgements} />
        {measurement.purged && <p className="replay-event-note">Host confirmed explicit purge. A new session is required before any countable interaction.</p>}
        {measurement.error && (
          <p className="measurement-error" role="alert">
            {measurement.error}
          </p>
        )}
        <AggregatePanel aggregate={measurement.aggregate} />
      </section>
      <RichReplay selected={selected} replay={replay} dispatchReplay={dispatchReplay} recordOutcome={recordOutcome} countable={active} setNotice={setNotice} />
    </>
  );
}

function EvidenceLedger({ evidence, viewState, dispatchView }) {
  if (!evidence || evidence.kind !== "simulated") {
    return (
      <section className="evidence-section evidence-unavailable" aria-label="Task evidence">
        <div className="section-heading">
          <h2>Evidence</h2>
          <span className="evidence-state">Not recorded</span>
        </div>
        <div className="evidence-empty">
          <Info size={18} />
          <div>
            <strong>No evidence record yet</strong>
            <span>This task will show files, checks, and usage once Spark records them.</span>
          </div>
        </div>
      </section>
    );
  }

  const tokenLine = evidence.tokens?.replace("Â·", "·");
  return (
    <>
      <section className="evidence-section" aria-label="Task evidence">
        <div className="section-heading">
          <h2>Changed files</h2>
          <span className="count">{evidence.files.length}</span>
        </div>
        <div className="file-ledger">
          {evidence.files.map(([path, added, removed, note]) => (
            <button
              className={`file-row ${viewState.focusedFile === path ? "focused" : ""}`}
              type="button"
              key={path}
              onClick={() => dispatchView({ type: "select-file", path })}
              aria-pressed={viewState.focusedFile === path}
            >
              <FileCode size={15} />
              <code>{path.replace(/^fixture\//, "src/")}</code>
              <span className="added">{added}</span>
              <span className="removed">{removed}</span>
              <span className="file-note">{note.replace(/^Simulated /, "")}</span>
            </button>
          ))}
        </div>
        {viewState.showFiles && viewState.focusedFile && (
          <div className="file-inspector" role="status">
            <div>
              <span>Selected evidence</span>
              <strong>{viewState.focusedFile.replace(/^fixture\//, "src/")}</strong>
            </div>
            <p>Change details stay in the review pane. This summary preserves the task-level proof without opening a full diff.</p>
          </div>
        )}
      </section>
      <section className="validation-section" aria-label="Validation evidence">
        <div className="section-heading validation-heading">
          <h2>Validation</h2>
          <span className="all-passed"><CheckCircle size={15} /> All recorded checks passed</span>
        </div>
        <div className="validation-list">
          {evidence.validations.map(([command, duration]) => (
            <div className="validation-row" key={command}>
              <CheckCircle size={15} weight="fill" />
              <code>{command}</code>
              <span>{duration}</span>
            </div>
          ))}
        </div>
      </section>
      <button type="button" className="how-worked-toggle" onClick={() => dispatchView({ type: "toggle-work" })} aria-expanded={viewState.showWork}>
        <CaretDown size={15} className={viewState.showWork ? "open" : ""} />
        <strong>How Spark worked</strong>
        <span>Model steps and tool calls</span>
        <small>8 steps</small>
      </button>
      {viewState.showWork && (
        <div className="work-details">
          {evidence.work.map(([label, text]) => (
            <div key={label}><span>{label}</span><strong>{text}</strong></div>
          ))}
        </div>
      )}
      {tokenLine && <p className="evidence-footnote">Source-reported usage: {tokenLine}. Pricing is unavailable.</p>}
    </>
  );
}

function ReviewPane({ evidence, onClose }) {
  if (!evidence || evidence.kind !== "simulated") return null;
  return (
    <aside className="review-pane" aria-label="Review changes">
      <div className="review-heading">
        <div><span>Review</span><strong>Changes</strong></div>
        <button type="button" className="icon-button" onClick={onClose} aria-label="Close review pane"><X size={17} /></button>
      </div>
      <p className="review-summary">A concise review ledger, kept beside the result so the task remains readable.</p>
      <div className="review-list">
        {evidence.files.map(([path, added, removed, note]) => (
          <div className="review-row" key={path}>
            <code>{path.replace(/^fixture\//, "src/")}</code>
            <span><b>{added}</b> <em>{removed}</em></span>
            <p>{note.replace(/^Simulated /, "")}</p>
          </div>
        ))}
      </div>
    </aside>
  );
}

export function App() {
  const [viewState, setViewState] = useState({ ...initialProoflineViewState, selectedId: "fork" });
  const [measurement, dispatchMeasurement] = useReducer(reduceWave1MeasurementState, initialWave1MeasurementState);
  const [composer, setComposer] = useState("");
  const [notice, setNotice] = useState("");
  const [model, setModel] = useState("GPT-5.3-Codex-Spark");
  const [reasoning, setReasoning] = useState("Medium");
  const [authorityMode, setAuthorityMode] = useState("ask");
  const [submittedRun, setSubmittedRun] = useState(null);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const taskRailRef = useRef(null);
  const composerRef = useRef(null);
  const submittedRunRef = useRef(null);
  const lifecycleAdapter = useMemo(() => createLifecycleAdapter(), []);
  const lifecycle = useMemo(() => createLifecycleReceiptController({ adapter: lifecycleAdapter }), [lifecycleAdapter]);
  const selected = getTaskFixture(viewState.selectedId);
  const isWave1 = Boolean(selected.scenario);
  const authority = authorityModes[authorityMode];
  const captureLabel = measurement.capture?.countable ? "Host measurement" : measurement.capture?.capture_mode === "host_authoritative" ? "Native preflight blocked" : "Browser rehearsal";
  const dispatchView = (event) => setViewState((state) => reduceProoflineViewState(state, event));
  const replayGroup = taskGroups.find((group) => group.tasks.some((task) => task.scenario));
  useEffect(() => {
    let alive = true;
    lifecycle
      .beginLaunch()
      .then(() => alive && lifecycle.acknowledgeUiReadyWhenInteractive({ taskRail: taskRailRef.current, composer: composerRef.current }))
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [lifecycle]);
  useEffect(() => {
    if (!submittedRun) return;
    lifecycle.acknowledgeFirstVisibleAfterFrames({ isVisible: () => submittedRunRef.current?.isConnected === true && submittedRunRef.current.dataset.runSubmitted === "true" });
  }, [lifecycle, submittedRun]);
  async function submitComposer(event) {
    event.preventDefault();
    if (!composer.trim()) return;
    if (lifecycleAdapter.kind === "browser") {
      setNotice(prototypeSubmissionNotice());
      setComposer("");
      return;
    }
    try {
      const accepted = await commitSubmittedRunAfterHostAck({
        controller: lifecycle,
        commit: (run) => {
          setNotice(prototypeSubmissionNotice());
          setComposer("");
          setSubmittedRun(run.challenge);
        },
      });
      if (!accepted) setNotice("Native lifecycle did not accept this staged instruction. No run timing was recorded.");
    } catch {
      setNotice("Native lifecycle did not accept this staged instruction. No run timing was recorded.");
    }
  }
  return (
    <main className="app-shell">
      <header className="window-bar">
        <div className="window-product">
          <img className="mini-mark" src={prooflineMark} alt="" />
          <span>Proofline for Spark</span>
        </div>
        <div className="window-controls" aria-label="Window controls">
          <button type="button" aria-label="Minimize">
            <Minus size={16} />
          </button>
          <button type="button" aria-label="Maximize">
            <Cube size={14} />
          </button>
          <button type="button" aria-label="Close">
            <X size={16} />
          </button>
        </div>
      </header>
      <div className="workspace-shell">
        <aside className="thread-rail" aria-label="Spark task history" ref={taskRailRef}>
          <div className="rail-brand">
            <img className="brand-image" src={prooflineMark} alt="" />
            <div>
              <strong>Proofline</strong>
              <span>for Spark</span>
            </div>
            <button type="button" className="icon-button new-task" aria-label="New task" onClick={() => setNotice("A fresh task is ready for your next instruction.")}>
              <PencilSimple size={19} />
            </button>
          </div>
          <nav>
            {taskGroups.filter((group) => !group.tasks.some((task) => task.scenario)).map((group) => (
              <div key={group.label}>
                <p className={`period-label ${group.label === "This week" ? "week-label" : ""}`}>{group.label}</p>
                <div className="thread-list">
                  {group.tasks.map((thread) => (
                    <ThreadItem
                      key={thread.id}
                      thread={thread}
                      active={viewState.selectedId === thread.id}
                      onClick={() => {
                        dispatchView({ type: "select-task", id: thread.id });
                        setNotice("");
                      }}
                    />
                  ))}
                </div>
              </div>
            ))}
          </nav>
          {replayGroup && (
            <details className="advanced-rail" open={advancedOpen} onToggle={(event) => setAdvancedOpen(event.currentTarget.open)}>
              <summary>Advanced replay and measurement</summary>
              <div className="thread-list">
                {replayGroup.tasks.map((thread) => (
                  <ThreadItem
                    key={thread.id}
                    thread={thread}
                    active={viewState.selectedId === thread.id}
                    onClick={() => {
                      dispatchView({ type: "select-task", id: thread.id });
                      setNotice("");
                    }}
                  />
                ))}
              </div>
            </details>
          )}
          <div className="rail-footer">
            <button type="button" className="icon-button" aria-label="Settings">
              <Gear size={18} />
            </button>
            <button type="button" className="icon-button" aria-label="Profile">
              <UserCircle size={19} />
            </button>
          </div>
        </aside>
        <section className={`task-surface ${viewState.reviewing ? "with-review" : ""}`}>
          <div className="task-content">
            <div className={`outcome-meta ${selected.complete ? "" : "in-progress"}`}>
              <CheckCircle size={19} weight="regular" />
              <strong>{selected.complete ? "COMPLETED" : "IN PROGRESS"}</strong>
              <span className="dot">•</span>
              <span>{selected.time}</span>
              <span className="dot">•</span>
              <span>{selected.elapsed}</span>
            </div>
            <h1>{selected.title}</h1>
            <p className="summary">{selected.summary}</p>
            {!isWave1 && (
              <>
                <div className="action-row evidence-actions">
                  <button type="button" className="action-button primary" onClick={() => dispatchView({ type: "toggle-review" })}>
                    <span>{viewState.reviewing ? "Close review" : "Review changes"}</span>
                    <ArrowRight size={18} />
                  </button>
                  <button type="button" className="action-button" onClick={() => composerRef.current?.focus()}>
                    <span>Continue</span>
                    <ArrowRight size={17} />
                  </button>
                  <button type="button" className="action-button" onClick={() => dispatchView({ type: "toggle-files" })}>
                    <span>{viewState.showFiles ? "Hide files" : "Open files"}</span>
                    <FileCode size={17} />
                  </button>
                </div>
                <EvidenceLedger evidence={selected.evidence} viewState={viewState} dispatchView={dispatchView} />
              </>
            )}
            <p className="prototype-boundary" role="note">
              <Info size={15} weight="fill" />
              {isWave1 ? "Browser and lifecycle-blocked native modes are rehearsal-only. No participant-countable Wave 1 session is available yet." : "Simulated prototype data — not connected to a Spark session, repository, or provider account."}
            </p>
            {isWave1 ? (
              <Wave1Measurement selected={selected} measurement={measurement} dispatch={dispatchMeasurement} setNotice={setNotice} />
            ) : (
              <section className="evidence-section evidence-unavailable">
                <div className="section-heading">
                  <h2>Evidence</h2>
                  <span className="evidence-state">Not recorded</span>
                </div>
                <div className="evidence-empty">
                  <Info size={18} />
                  <div>
                    <strong>No detailed fixture evidence</strong>
                    <span>This task is deliberately unavailable until a fixture or host record exists.</span>
                  </div>
                </div>
              </section>
            )}
            {notice && (
              <div className="action-notice" role="status" ref={submittedRun ? submittedRunRef : null} data-run-submitted={submittedRun ? "true" : undefined} style={isWave1 ? { marginTop: 8 } : undefined}>
                <CheckCircle size={16} />
                {notice}
              </div>
            )}
            <form className="composer" onSubmit={submitComposer}>
              <textarea ref={composerRef} value={composer} onChange={(event) => setComposer(event.target.value)} placeholder="What should Spark do next?" aria-label="What should Spark do next?" />
              <p className="authority-note">
                <ShieldCheck size={15} />
                {authority.note}
              </p>
              <div className="composer-footer">
                <label>
                  Model
                  <select value={model} onChange={(event) => setModel(event.target.value)}>
                    <option>GPT-5.3-Codex-Spark</option>
                    <option>GPT-5.6-Luna</option>
                  </select>
                  <CaretDown size={14} />
                </label>
                <label>
                  Reasoning
                  <select value={reasoning} onChange={(event) => setReasoning(event.target.value)}>
                    <option>Low</option>
                    <option>Medium</option>
                    <option>High</option>
                  </select>
                  <CaretDown size={14} />
                </label>
                <label>
                  Mode
                  <select value={authorityMode} onChange={(event) => setAuthorityMode(event.target.value)}>
                    {Object.entries(authorityModes).map(([value, option]) => (
                      <option value={value} key={value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                  <CaretDown size={14} />
                </label>
                <button className="send-button" type="submit" aria-label="Stage prototype instruction">
                  <PaperPlaneTilt size={20} weight="bold" />
                </button>
              </div>
            </form>
          </div>
          {viewState.reviewing && <ReviewPane evidence={selected.evidence} onClose={() => dispatchView({ type: "toggle-review" })} />}
        </section>
      </div>
      <footer className="status-ribbon">
        <span>
          <GitBranch size={17} />
          {isWave1 ? captureLabel : "main"}
        </span>
        <i />
        <span>{isWave1 ? (measurement.capture?.fixture?.verified ? "Fixture evidence verified" : "Host verification required") : selected.evidence?.checkpoint ? `Checkpoint ${selected.evidence.checkpoint}` : "Checkpoint unavailable"}</span>
        <i />
        <span>
          <Clock size={17} />
          Elapsed {selected.elapsed}
        </span>
        <i />
        <span>
          <Cube size={17} />
          {isWave1 ? "Usage display only" : String(selected.evidence?.tokens ?? "Tokens unavailable").replace(/[^\x20-\x7e]/g, " ").replace(/\s+/g, " ").trim()}
        </span>
        <i />
        <span>
          <Tag size={17} />
          Pricing unavailable
        </span>
        <span className="status-spacer" />
        <span className="local">
          <b />
          Local-first
        </span>
        <span>
          <ShieldCheck size={17} />
          Private
        </span>
      </footer>
    </main>
  );
}
