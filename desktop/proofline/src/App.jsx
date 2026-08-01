import { useReducer, useState } from "react";
import {
  ArrowRight,
  CaretDown,
  CaretRight,
  CheckCircle,
  Circle,
  Clock,
  Cube,
  FileCode,
  FolderSimple,
  Gear,
  GitBranch,
  Info,
  Minus,
  PaperPlaneTilt,
  PencilSimple,
  ShieldCheck,
  Sparkle,
  Tag,
  UserCircle,
  X,
} from "@phosphor-icons/react";
import {
  authorityModes,
  getTaskFixture,
  hasDetailedEvidence,
  initialProoflineViewState,
  prototypeSubmissionNotice,
  reduceProoflineViewState,
  taskGroups,
} from "./proofline-state.js";
import {
  createRedactedWave1Export,
  getWave1Scenario,
  initialWave1ReplayState,
  reduceWave1ReplayState,
  WAVE1_FIXTURE_ID,
  WAVE1_FIXTURE_REVISION,
  WAVE1_FIXTURE_SHA256,
} from "./wave1-replay.js";

function ThreadItem({ thread, active, onClick }) {
  return (
    <button className={`thread-item ${active ? "selected" : ""}`} onClick={onClick} type="button">
      <span className="thread-copy">
        <span className="thread-title">{thread.title}</span>
        <span className="thread-meta">{thread.time}<span className="dot">•</span>{thread.elapsed}</span>
      </span>
      {thread.complete ? <CheckCircle size={17} weight="regular" /> : <Circle size={17} weight="regular" />}
    </button>
  );
}

function ActionButton({ children, icon: Icon, onClick, primary = false, disabled = false, title }) {
  return (
    <button type="button" className={`action-button ${primary ? "primary" : ""}`} disabled={disabled} onClick={onClick} title={title}>
      <span>{children}</span><Icon size={19} weight="regular" />
    </button>
  );
}

function PrototypeBoundary() {
  return (
    <p className="prototype-boundary" role="note">
      <Info size={15} weight="fill" />
      Simulated prototype data — not connected to a Spark session, repository, or provider account.
    </p>
  );
}

function UnavailableEvidence({ task }) {
  return (
    <section className="evidence-section evidence-unavailable" aria-labelledby="evidence-heading">
      <div className="section-heading"><h2 id="evidence-heading">Evidence</h2><span className="evidence-state">Not recorded</span></div>
      <div className="evidence-empty" role="status">
        <Info size={18} weight="regular" />
        <div><strong>No detailed fixture evidence</strong><span>{task.evidence.message} Files, validation, work steps, checkpoint, and token counts are unavailable for this task.</span></div>
      </div>
    </section>
  );
}

function SimulatedEvidence({ task, viewState, dispatchView, setNotice }) {
  const { evidence } = task;
  const focusedFile = viewState.focusedFile ?? evidence.files[0][0];

  return (
    <>
      <section className="evidence-section" aria-labelledby="changed-files-heading">
        <div className="section-heading"><h2 id="changed-files-heading">Simulated changed files</h2><span className="count">{evidence.files.length}</span><span className="fixture-label">Fixture only</span></div>
        <div className="file-ledger">
          {evidence.files.map(([path, added, removed, explanation]) => (
            <button className="file-row" type="button" key={path} onClick={() => { dispatchView({ type: "select-file", path }); setNotice(`Opened simulated file inspector for ${path}.`); }}>
              <FileCode size={15} weight="regular" /><code>{path}</code><span className="added">{added}</span><span className="removed">{removed}</span><span className="file-note">{explanation}</span>
            </button>
          ))}
        </div>
        {viewState.showFiles && <aside className="file-inspector" aria-label="Simulated file inspector">
          <div><span>Simulated file inspector</span><strong>{focusedFile}</strong></div>
          <p>This fixture demonstrates the review anatomy only. It does not open a local file or expose a real diff.</p>
        </aside>}
      </section>

      <section className="validation-section" aria-labelledby="validation-heading">
        <div className="section-heading validation-heading"><h2 id="validation-heading">Simulated validation</h2><span className="all-passed"><CheckCircle size={16} />Fixture checks passed</span></div>
        <div className="validation-list">
          {evidence.validations.map(([command, duration]) => <button type="button" className="validation-row" key={command} onClick={() => setNotice(`Selected simulated validation output: ${command}`)}><CheckCircle size={16} /><code>{command}</code><span>{duration}</span></button>)}
        </div>
        <button type="button" className="output-link" onClick={() => setNotice("This fixture has no local command output to open.")}>About this fixture <ArrowRight size={15} /></button>
      </section>

      <section className={`how-worked ${viewState.showWork ? "open" : ""}`}>
        <button type="button" className="how-worked-toggle" onClick={() => dispatchView({ type: "toggle-work" })} aria-expanded={viewState.showWork}>
          {viewState.showWork ? <CaretDown size={18} /> : <CaretRight size={18} />}<strong>How the fixture is structured</strong><span>Simulated plan, evidence, and validation</span><small>{evidence.work.length} steps</small>
        </button>
        {viewState.showWork && <div className="work-details">{evidence.work.map(([label, description]) => <div key={label}><strong>{label}</strong><span>{description}</span></div>)}</div>}
      </section>
    </>
  );
}

function ReplayEventNote({ eventCount }) {
  return <p className="replay-event-note" role="status">Local fixture interactions recorded: {eventCount}. Event payloads exclude fixture content, prompts, paths, commands, diffs, raw tokens, identities, and transport.</p>;
}

function ReplayFixtureControls({ selected, replayState, dispatchReplay, setNotice }) {
  const redacted = createRedactedWave1Export(replayState.events);
  const act = (action, notice) => {
    dispatchReplay({ ...action, scenario: selected.scenario });
    setNotice(notice);
  };
  return <section className="replay-fixture-controls" aria-label="Local replay fixture controls">
    <div><span>Local replay fixture</span><strong>{WAVE1_FIXTURE_ID}</strong><small>Revision {WAVE1_FIXTURE_REVISION} · SHA-256 {WAVE1_FIXTURE_SHA256.slice(0, 12)}…</small></div>
    <p>Deterministic, local-only fixture. The manifest identity is displayed for facilitator preflight; this browser prototype does not verify an archive or collect launch samples.</p>
    <div className="replay-actions"><button type="button" className="action-button primary" disabled={replayState.runStarted} onClick={() => act({ type: "start-replay" }, "Replay submitted; first visible fixture activity rendered after 200 ms.")}>{replayState.runStarted ? "Replay activity rendered" : "Start selected replay"} <ArrowRight size={17} /></button><button type="button" className="action-button" onClick={() => act({ type: "view-redacted-export" }, "Redacted aggregate preview prepared locally.")}>Preview redacted export <ArrowRight size={17} /></button><button type="button" className="action-button" onClick={() => act({ type: "reset-local-replay" }, "Fixture state reset locally. No external profile or repository was changed.")}>Reset fixture <ArrowRight size={17} /></button><button type="button" className="action-button" onClick={() => act({ type: "purge-local-events" }, "In-memory replay events purged. This prototype has no persistent event store.")}>Purge local events <X size={17} /></button></div>
    {replayState.exportViewed && <div className="replay-export" role="status"><strong>Redacted aggregate preview</strong><span>Fixture {redacted.fixture.revision}; {redacted.event_count} local event categories across {redacted.event_counts_by_task.length} task(s). No event IDs, timestamps, prompts, fixture paths, commands, diffs, raw tokens, identities, or transport details are exported.</span></div>}
    {replayState.purged && <p className="replay-event-note">Local in-memory replay events are empty after purge.</p>}
  </section>;
}

function Wave1Replay({ selected, replayState, dispatchReplay, setNotice }) {
  const scenario = getWave1Scenario(selected.scenario);
  const act = (action, notice) => {
    dispatchReplay({ ...action, scenario: selected.scenario });
    setNotice(notice);
  };

  if (selected.scenario === "repo-brief") {
    return <section className="replay-section" aria-label="Wave 1 Task 1 replay">
      <div className="section-heading"><h2>Repo Brief evidence</h2><span className="fixture-label">Task 1 of 5</span></div>
      <div className="replay-card"><p>Likely ownership boundary: <strong>{scenario.answer}</strong></p><button className="action-button primary" type="button" disabled={!replayState.runStarted} onClick={() => act({ type: "open-repo-evidence" }, "Opened the cited fixture evidence.")}>Open cited evidence <ArrowRight size={18} /></button></div>
      {replayState.openedRepoEvidence && <div className="replay-detail"><strong>{scenario.citation.source}</strong><p>{scenario.citation.excerpt}</p><p><b>Uncertainty:</b> {scenario.uncertainty}</p><p><b>Next check:</b> {scenario.nextCheck}</p></div>}
      <ReplayEventNote eventCount={replayState.events.length} />
    </section>;
  }

  if (selected.scenario === "completed-change") {
    return <section className="replay-section" aria-label="Wave 1 Task 2 replay">
      <div className="section-heading"><h2>Completed fixture evidence</h2><span className="fixture-label">Task 2 of 5</span></div>
      <div className="file-ledger">{scenario.files.map((file) => <button className="file-row" type="button" key={file} onClick={() => act({ type: "open-diff" }, "Opened the simulated two-file change.")}><FileCode size={15} /><code>{file}</code><span className="added">fixture</span><span className="removed">only</span><span className="file-note">Simulated changed-file record</span></button>)}</div>
      <div className="replay-detail"><p><CheckCircle size={16} /> <b>{scenario.validation}</b></p><p>Checkpoint: <strong>{scenario.checkpoint}</strong></p>{replayState.openedDiff && <p>Simulated diff inspection is open. This fixture does not read a local repository.</p>}</div>
      <ReplayEventNote eventCount={replayState.events.length} />
    </section>;
  }

  if (selected.scenario === "failed-validation") {
    return <section className="replay-section" aria-label="Wave 1 Task 3 replay">
      <div className="section-heading"><h2>Failed fixture validation</h2><span className="fixture-label">Task 3 of 5</span></div>
      <div className="replay-detail replay-failure"><p><b>{scenario.validation}</b></p><p>{scenario.failure}</p><p><b>Simulated command:</b> <code>{scenario.failureCommand}</code></p><p><b>Simulated result:</b> {scenario.failureOutput}</p><p>This is a failed run state, not a completed result.</p></div>
      <div className="replay-actions">{scenario.choices.map((choice) => <button type="button" className="action-button" key={choice} onClick={() => act({ type: "recover", choice }, `${choice} selected in the simulated replay.`)}>{choice} <ArrowRight size={17} /></button>)}</div>
      {replayState.recoveryChoice && <p className="replay-detail"><b>Recorded recovery choice:</b> {replayState.recoveryChoice}. No validation, retry, restore, or repository mutation ran.</p>}
      <ReplayEventNote eventCount={replayState.events.length} />
    </section>;
  }

  if (selected.scenario === "pending-approval") {
    return <section className="replay-section" aria-label="Wave 1 Task 4 replay">
      <div className="section-heading"><h2>Pending fixture approval</h2><span className="fixture-label">Task 4 of 5</span></div>
      <div className="replay-detail replay-approval"><p><b>Run status: awaiting approval</b></p><p>Requested authority: fixture file change.</p><p><b>Policy card:</b> {scenario.policy}</p></div>
      <div className="replay-actions"><button type="button" className="action-button primary" onClick={() => act({ type: "decide-approval", decision: "approve" }, "Fixture file-change approval recorded locally.")}>Approve fixture change <CheckCircle size={18} /></button><button type="button" className="action-button" onClick={() => act({ type: "decide-approval", decision: "deny" }, "Fixture approval denial recorded locally.")}>Deny request <X size={18} /></button></div>
      {replayState.approvalDecision && <p className="replay-detail"><b>Policy outcome:</b> {replayState.approvalDecision === "approve" ? "approved fixture-only file change" : "denied request"}. No authority was granted.</p>}
      <ReplayEventNote eventCount={replayState.events.length} />
    </section>;
  }

  return <section className="replay-section" aria-label="Wave 1 Task 5 replay">
    <div className="section-heading"><h2>Usage and handoff boundary</h2><span className="fixture-label">Task 5 of 5</span></div>
    <div className="replay-detail"><p><b>Source-reported fixture tokens:</b> {scenario.sourceReportedTokens.input.toLocaleString()} input · {scenario.sourceReportedTokens.output.toLocaleString()} output.</p><p><b>Usage coverage:</b> Partial source-reported history; this is not quota.</p><p><b>Pricing:</b> Unavailable. This does not mean free or complete.</p><p>{scenario.handoff}</p></div>
    <button type="button" className="action-button primary" onClick={() => act({ type: "view-usage" }, "Partial usage and unavailable pricing were reviewed.")}>{replayState.usageViewed ? "Usage boundary reviewed" : "Review usage boundary"} <ArrowRight size={18} /></button>
    <ReplayEventNote eventCount={replayState.events.length} />
  </section>;
}

export function App() {
  const [viewState, setViewState] = useState(initialProoflineViewState);
  const [wave1State, dispatchWave1] = useReducer(reduceWave1ReplayState, undefined, initialWave1ReplayState);
  const [composer, setComposer] = useState("");
  const [notice, setNotice] = useState("");
  const [model, setModel] = useState("GPT-5.3-Codex-Spark");
  const [reasoning, setReasoning] = useState("Medium");
  const [authorityMode, setAuthorityMode] = useState("ask");
  const selected = getTaskFixture(viewState.selectedId);
  const isWave1Replay = Boolean(selected.scenario);
  const detailed = hasDetailedEvidence(selected);
  const authority = authorityModes[authorityMode];
  const dispatchView = (event) => setViewState((state) => reduceProoflineViewState(state, event));

  function chooseThread(id) {
    dispatchView({ type: "select-task", id });
    setNotice("");
  }

  function submitComposer(event) {
    event.preventDefault();
    if (!composer.trim()) return;
    setNotice(prototypeSubmissionNotice());
    setComposer("");
  }

  function toggleReview() {
    const opening = !viewState.reviewing;
    dispatchView({ type: "toggle-review" });
    setNotice(opening ? "Simulated review mode is open: inspect the fixture evidence below." : "Review closed. Spark remains ready.");
  }

  function toggleFiles() {
    const opening = !viewState.showFiles;
    dispatchView({ type: "toggle-files" });
    setNotice(opening ? "Simulated file inspector opened below the fixture ledger." : "Simulated file inspector closed.");
  }

  return (
    <main className="app-shell">
      <header className="window-bar">
        <div className="window-product"><span className="mini-mark"><Sparkle size={15} weight="fill" /></span><span>Proofline for Spark</span></div>
        <div className="window-controls" aria-label="Window controls"><button type="button" aria-label="Minimize"><Minus size={16} /></button><button type="button" aria-label="Maximize"><Cube size={14} /></button><button type="button" aria-label="Close"><X size={16} /></button></div>
      </header>

      <div className="workspace-shell">
        <aside className="thread-rail" aria-label="Spark task history">
          <div className="rail-brand">
            <Sparkle className="brand-mark" size={42} weight="regular" />
            <div><strong>Proofline</strong><span>for Spark</span></div>
            <button type="button" className="icon-button new-task" aria-label="New task" onClick={() => setNotice("A fresh task is ready for your next instruction.")}><PencilSimple size={19} /></button>
          </div>
          <nav>
            {taskGroups.map((group) => <div key={group.label}><p className={`period-label ${group.label === "This week" ? "week-label" : ""}`}>{group.label}</p><div className="thread-list">{group.tasks.map((thread) => <ThreadItem key={thread.id} thread={thread} active={viewState.selectedId === thread.id} onClick={() => chooseThread(thread.id)} />)}</div></div>)}
          </nav>
          <div className="rail-footer"><button type="button" className="icon-button" aria-label="Settings"><Gear size={18} /></button><button type="button" className="icon-button" aria-label="Profile"><UserCircle size={19} /></button></div>
        </aside>

        <section className="task-surface">
          <div className="task-content">
            <div className={`outcome-meta ${selected.complete ? "" : "in-progress"}`}><CheckCircle size={19} weight="regular" /><strong>{viewState.reviewing ? "REVIEWING" : selected.complete ? "COMPLETED" : "IN PROGRESS"}</strong><span className="dot">•</span><span>{selected.time}</span><span className="dot">•</span><span>{selected.elapsed}</span></div>
            <h1>{selected.title}</h1>
            <p className="summary">{selected.summary}</p>
            <PrototypeBoundary />

            {isWave1Replay ? <><ReplayFixtureControls selected={selected} replayState={wave1State} dispatchReplay={dispatchWave1} setNotice={setNotice} /><Wave1Replay selected={selected} replayState={wave1State} dispatchReplay={dispatchWave1} setNotice={setNotice} /></> : <><div className="action-row">
              <ActionButton icon={ArrowRight} primary disabled={!detailed} title={detailed ? undefined : "No detailed fixture evidence is recorded for this task."} onClick={toggleReview}>{viewState.reviewing ? "Close review" : detailed ? "Review fixture" : "Review unavailable"}</ActionButton>
              <ActionButton icon={ArrowRight} onClick={() => { setComposer(`Continue ${selected.title.toLowerCase()} with `); setNotice("Continuation prepared in the composer."); }}>Continue</ActionButton>
              <ActionButton icon={FolderSimple} disabled={!detailed} title={detailed ? undefined : "No detailed fixture files are recorded for this task."} onClick={toggleFiles}>{viewState.showFiles ? "Close files" : detailed ? "Open files" : "Files unavailable"}</ActionButton>
            </div>

            {notice && <div className="action-notice" role="status"><CheckCircle size={16} />{notice}</div>}

            {detailed ? <SimulatedEvidence task={selected} viewState={viewState} dispatchView={dispatchView} setNotice={setNotice} /> : <UnavailableEvidence task={selected} />}</>}

            {isWave1Replay && notice && <div className="action-notice" role="status"><CheckCircle size={16} />{notice}</div>}

            <form className="composer" onSubmit={submitComposer}>
              <textarea value={composer} onChange={(event) => setComposer(event.target.value)} placeholder="What should Spark do next?" aria-label="What should Spark do next?" />
              <p className="authority-note"><ShieldCheck size={15} />{authority.note}</p>
              <div className="composer-footer">
                <label>Model<select value={model} onChange={(event) => setModel(event.target.value)}><option>GPT-5.3-Codex-Spark</option><option>GPT-5.6-Luna</option></select><CaretDown size={14} /></label>
                <label>Reasoning<select value={reasoning} onChange={(event) => setReasoning(event.target.value)}><option>Low</option><option>Medium</option><option>High</option></select><CaretDown size={14} /></label>
                <label>Mode<select value={authorityMode} onChange={(event) => setAuthorityMode(event.target.value)}>{Object.entries(authorityModes).map(([value, option]) => <option value={value} key={value}>{option.label}</option>)}</select><CaretDown size={14} /></label>
                <button className="send-button" type="submit" aria-label="Send to Spark"><PaperPlaneTilt size={20} weight="bold" /></button>
              </div>
            </form>
          </div>
        </section>
      </div>

      <footer className="status-ribbon">
        <span><GitBranch size={17} />{isWave1Replay ? "Local replay fixture" : "Fixture branch  main"}</span><i /><span>{detailed ? <>Fixture checkpoint&nbsp; {selected.evidence.checkpoint}</> : isWave1Replay ? "No repository checkpoint" : "Checkpoint unavailable"}</span><i /><span><Clock size={17} />Elapsed&nbsp; {selected.elapsed}</span><i /><span><Cube size={17} />{detailed ? <>Fixture tokens&nbsp; {selected.evidence.tokens}</> : selected.scenario === "partial-usage" ? <>Source-reported fixture tokens&nbsp; {selected.evidence?.tokens ?? "18,742 in · 4,396 out"}</> : "Tokens unavailable"}</span><i /><span><Tag size={17} />Pricing&nbsp; Unavailable</span><span className="status-spacer" /><span className="local"><b />Local-first</span><span><ShieldCheck size={17} />Mode shown</span>
      </footer>
    </main>
  );
}
