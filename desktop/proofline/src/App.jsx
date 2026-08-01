import { useMemo, useState } from "react";
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
  Minus,
  PaperPlaneTilt,
  PencilSimple,
  ShieldCheck,
  Sparkle,
  Tag,
  UserCircle,
  X,
} from "@phosphor-icons/react";

const today = [
  { id: "fork", title: "Add fork-aware usage history", time: "10:42 AM", elapsed: "31s", complete: true },
  { id: "errors", title: "Improve error messaging", time: "9:58 AM", elapsed: "18s", complete: true },
  { id: "sessions", title: "Refactor session store", time: "9:22 AM", elapsed: "27s", complete: true },
  { id: "quota", title: "Tighten quota validation", time: "8:47 AM", elapsed: "21s", complete: false },
];

const week = [
  { id: "json", title: "Add export to JSON", time: "Yesterday", elapsed: "35s", complete: true },
  { id: "indexes", title: "Optimize index queries", time: "Yesterday", elapsed: "42s", complete: true },
  { id: "retention", title: "Introduce retention job", time: "Mon", elapsed: "29s", complete: false },
  { id: "integration", title: "Add integration tests", time: "Mon", elapsed: "52s", complete: true },
  { id: "admin", title: "Scaffold admin commands", time: "Mon", elapsed: "16s", complete: false },
];

const files = [
  ["src/usage/history.rs", "+182", "-10", "Fork-aware history queries and lineage resolution"],
  ["src/usage/record.rs", "+64", "-6", "Record fork_id and parent_run_id on usage events"],
  ["src/db/schema.rs", "+38", "-2", "Add fork_id to usage_history table and indexes"],
  ["migrations/20240521_add_fork_id.sql", "+26", "-0", "Schema migration for fork_id column and index"],
  ["src/api/history.rs", "+41", "-3", "Expose fork-aware history in API responses"],
  ["tests/usage_history_fork.rs", "+96", "-1", "Integration tests for fork lineage and attribution"],
];

const validations = [
  ["cargo fmt --all -- --check", "2s"],
  ["cargo clippy --workspace --all-targets -- -D warnings", "6s"],
  ["cargo test --workspace --all-features", "19s"],
  ["cargo test --test usage_history_fork", "3s"],
];

const detailByThread = {
  fork: {
    title: "Add fork-aware usage history",
    summary: "Spark added fork-aware usage history so usage tracking correctly attributes source-reported tokens to the originating fork lineage. History queries now include fork context; pricing remains unavailable.",
    time: "10:42 AM",
    elapsed: "31s",
    complete: true,
  },
  errors: { title: "Improve error messaging", summary: "Spark clarified recoverable command failures with direct next steps and preserved the original diagnostic context.", time: "9:58 AM", elapsed: "18s", complete: true },
  sessions: { title: "Refactor session store", summary: "Spark separated session metadata from transcript persistence while maintaining checkpoint compatibility.", time: "9:22 AM", elapsed: "27s", complete: true },
  quota: { title: "Tighten quota validation", summary: "Spark is checking quota boundaries before a provider request is created.", time: "8:47 AM", elapsed: "21s", complete: false },
  json: { title: "Add export to JSON", summary: "Spark prepared a portable local evidence export with explicit schema and provenance fields.", time: "Yesterday", elapsed: "35s", complete: true },
  indexes: { title: "Optimize index queries", summary: "Spark reduced redundant history lookups while preserving the current result ordering.", time: "Yesterday", elapsed: "42s", complete: true },
  retention: { title: "Introduce retention job", summary: "Spark is defining a local retention policy and a visible purge boundary before implementation.", time: "Mon", elapsed: "29s", complete: false },
  integration: { title: "Add integration tests", summary: "Spark added coverage for session lineage, validation evidence, and unavailable pricing states.", time: "Mon", elapsed: "52s", complete: true },
  admin: { title: "Scaffold admin commands", summary: "Spark is separating privileged maintenance commands from the default interactive surface.", time: "Mon", elapsed: "16s", complete: false },
};

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

function ActionButton({ children, icon: Icon, onClick, primary = false }) {
  return (
    <button type="button" className={`action-button ${primary ? "primary" : ""}`} onClick={onClick}>
      <span>{children}</span><Icon size={19} weight="regular" />
    </button>
  );
}

export function App() {
  const [selectedId, setSelectedId] = useState("fork");
  const [showWork, setShowWork] = useState(false);
  const [showFiles, setShowFiles] = useState(false);
  const [reviewing, setReviewing] = useState(false);
  const [composer, setComposer] = useState("");
  const [notice, setNotice] = useState("");
  const [model, setModel] = useState("GPT-5.3-Codex-Spark");
  const [reasoning, setReasoning] = useState("Medium");
  const [workspace, setWorkspace] = useState("Local (full access)");
  const selected = useMemo(() => detailByThread[selectedId] ?? detailByThread.fork, [selectedId]);

  function chooseThread(id) {
    setSelectedId(id);
    setNotice("");
    setReviewing(false);
  }

  function submitComposer(event) {
    event.preventDefault();
    const text = composer.trim();
    if (!text) return;
    setNotice(`Queued for Spark: ${text}`);
    setComposer("");
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
            <p className="period-label">Today</p>
            <div className="thread-list">{today.map((thread) => <ThreadItem key={thread.id} thread={thread} active={selectedId === thread.id} onClick={() => chooseThread(thread.id)} />)}</div>
            <p className="period-label week-label">This week</p>
            <div className="thread-list">{week.map((thread) => <ThreadItem key={thread.id} thread={thread} active={selectedId === thread.id} onClick={() => chooseThread(thread.id)} />)}</div>
          </nav>
          <div className="rail-footer"><button type="button" className="icon-button" aria-label="Settings"><Gear size={18} /></button><button type="button" className="icon-button" aria-label="Profile"><UserCircle size={19} /></button></div>
        </aside>

        <section className="task-surface">
          <div className="task-content">
            <div className={`outcome-meta ${selected.complete ? "" : "in-progress"}`}><CheckCircle size={19} weight="regular" /><strong>{reviewing ? "REVIEWING" : selected.complete ? "COMPLETED" : "IN PROGRESS"}</strong><span className="dot">•</span><span>{selected.time}</span><span className="dot">•</span><span>{selected.elapsed}</span></div>
            <h1>{selected.title}</h1>
            <p className="summary">{selected.summary}</p>

            <div className="action-row">
              <ActionButton icon={ArrowRight} primary onClick={() => { setReviewing((value) => !value); setNotice(reviewing ? "Review closed. Spark remains ready." : "Review mode is open: inspect every changed file below."); }}>{reviewing ? "Close review" : "Review changes"}</ActionButton>
              <ActionButton icon={ArrowRight} onClick={() => { setComposer(`Continue ${selected.title.toLowerCase()} with `); setNotice("Continuation prepared in the composer."); }}>Continue</ActionButton>
              <ActionButton icon={FolderSimple} onClick={() => { setShowFiles((value) => !value); setNotice(showFiles ? "Changed-files drawer closed." : "Changed files expanded for inspection."); }}>{showFiles ? "Close files" : "Open files"}</ActionButton>
            </div>

            {notice && <div className="action-notice" role="status"><CheckCircle size={16} />{notice}</div>}

            <section className="evidence-section" aria-labelledby="changed-files-heading">
              <div className="section-heading"><h2 id="changed-files-heading">Changed files</h2><span className="count">{files.length}</span></div>
              <div className={`file-ledger ${showFiles ? "expanded" : ""}`}>
                {files.map(([path, added, removed, explanation]) => (
                  <button className="file-row" type="button" key={path} onClick={() => setNotice(`Selected ${path} for review.`)}>
                    <FileCode size={15} weight="regular" /><code>{path}</code><span className="added">{added}</span><span className="removed">{removed}</span><span className="file-note">{explanation}</span>
                  </button>
                ))}
              </div>
            </section>

            <section className="validation-section" aria-labelledby="validation-heading">
              <div className="section-heading validation-heading"><h2 id="validation-heading">Validation</h2><span className="all-passed"><CheckCircle size={16} />All checks passed</span></div>
              <div className="validation-list">
                {validations.map(([command, duration]) => <button type="button" className="validation-row" key={command} onClick={() => setNotice(`Validation output selected: ${command}`)}><CheckCircle size={16} /><code>{command}</code><span>{duration}</span></button>)}
              </div>
              <button type="button" className="output-link" onClick={() => setNotice("Full output is available in the local evidence drawer.")}>View full output <ArrowRight size={15} /></button>
            </section>

            <section className={`how-worked ${showWork ? "open" : ""}`}>
              <button type="button" className="how-worked-toggle" onClick={() => setShowWork((value) => !value)} aria-expanded={showWork}>
                {showWork ? <CaretDown size={18} /> : <CaretRight size={18} />}<strong>How Spark worked</strong><span>Model steps, tool calls, and reasoning {showWork ? "(expanded)" : "(collapsed)"}</span><small>8 steps</small>
              </button>
              {showWork && <div className="work-details"><div><strong>Plan</strong><span>Resolve usage lineage before aggregating token evidence.</span></div><div><strong>Evidence</strong><span>Read session traces, deduplicated replayed events, and retained source boundaries.</span></div><div><strong>Validation</strong><span>Ran the recorded Rust checks and preserved their results with this task.</span></div></div>}
            </section>

            <form className="composer" onSubmit={submitComposer}>
              <textarea value={composer} onChange={(event) => setComposer(event.target.value)} placeholder="What should Spark do next?" aria-label="What should Spark do next?" />
              <div className="composer-footer">
                <label>Model<select value={model} onChange={(event) => setModel(event.target.value)}><option>GPT-5.3-Codex-Spark</option><option>GPT-5.6-Luna</option></select><CaretDown size={14} /></label>
                <label>Reasoning<select value={reasoning} onChange={(event) => setReasoning(event.target.value)}><option>Low</option><option>Medium</option><option>High</option></select><CaretDown size={14} /></label>
                <label>Workspace<select value={workspace} onChange={(event) => setWorkspace(event.target.value)}><option>Local (full access)</option><option>Workspace only</option><option>Read only</option></select><CaretDown size={14} /></label>
                <button className="send-button" type="submit" aria-label="Send to Spark"><PaperPlaneTilt size={20} weight="bold" /></button>
              </div>
            </form>
          </div>
        </section>
      </div>

      <footer className="status-ribbon">
        <span><GitBranch size={17} />main</span><i /><span>Checkpoint&nbsp; 23f7c9a</span><i /><span><Clock size={17} />Elapsed&nbsp; {selected.elapsed}</span><i /><span><Cube size={17} />Tokens&nbsp; 18,742 in · 4,396 out</span><i /><span><Tag size={17} />Pricing&nbsp; Unavailable</span><span className="status-spacer" /><span className="local"><b />Local-first</span><span><ShieldCheck size={17} />Permissions shown</span>
      </footer>
    </main>
  );
}
