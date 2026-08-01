import assert from "node:assert/strict";
import test from "node:test";
import {
  authorityModes,
  getTaskFixture,
  hasDetailedEvidence,
  initialProoflineViewState,
  prototypeSubmissionNotice,
  reduceProoflineViewState,
} from "../src/proofline-state.js";

test("Wave 1 replay tasks can render without legacy detailed evidence", () => {
  const replayTask = getTaskFixture("repo-brief");
  assert.equal(replayTask.scenario, "repo-brief");
  assert.equal(hasDetailedEvidence(replayTask), false);
});

test("non-fork tasks do not inherit simulated fork evidence", () => {
  const errors = getTaskFixture("errors");

  assert.equal(errors.evidence.kind, "unavailable");
  assert.deepEqual(errors.evidence.files, []);
  assert.deepEqual(errors.evidence.validations, []);
  assert.equal(errors.evidence.checkpoint, null);
  assert.equal(errors.evidence.tokens, null);

  const afterSelection = reduceProoflineViewState(
    { ...initialProoflineViewState, showFiles: true, reviewing: true, focusedFile: "fixture/usage/history.rs" },
    { type: "select-task", id: "errors" },
  );
  assert.deepEqual(afterSelection, { ...initialProoflineViewState, selectedId: "errors" });
  assert.equal(reduceProoflineViewState(afterSelection, { type: "toggle-review" }), afterSelection);
  assert.equal(reduceProoflineViewState(afterSelection, { type: "toggle-files" }), afterSelection);
});

test("file inspector state visibly opens, focuses a fixture file, and closes", () => {
  const forkState = reduceProoflineViewState(initialProoflineViewState, { type: "select-task", id: "fork" });
  const opened = reduceProoflineViewState(forkState, { type: "toggle-files" });
  assert.equal(opened.showFiles, true);

  const focused = reduceProoflineViewState(opened, { type: "select-file", path: "fixture/usage/history.rs" });
  assert.equal(focused.showFiles, true);
  assert.equal(focused.focusedFile, "fixture/usage/history.rs");

  const closed = reduceProoflineViewState(focused, { type: "toggle-files" });
  assert.equal(closed.showFiles, false);
  assert.equal(closed.focusedFile, null);
});

test("the initial detailed fixture can enter and exit review mode", () => {
  const forkState = reduceProoflineViewState(initialProoflineViewState, { type: "select-task", id: "fork" });
  const reviewing = reduceProoflineViewState(forkState, { type: "toggle-review" });
  assert.equal(reviewing.reviewing, true);

  const closed = reduceProoflineViewState(reviewing, { type: "toggle-review" });
  assert.equal(closed.reviewing, false);
});

test("authority labels describe capability without promising full access or sandboxing", () => {
  assert.equal(authorityModes.ask.label, "Ask (read-only tools)");
  assert.equal(authorityModes.work.label, "Work (OS-user access)");
  assert.match(authorityModes.ask.note, /not a privacy or sandbox guarantee/i);
  assert.match(authorityModes.work.note, /OS-user access/i);
  assert.match(authorityModes.work.note, /not sandboxed/i);
  assert.doesNotMatch(authorityModes.work.label, /full access/i);
});

test("composer feedback never claims to send a live Spark request", () => {
  const notice = prototypeSubmissionNotice();

  assert.match(notice, /prototype instruction staged/i);
  assert.match(notice, /no request was sent/i);
  assert.doesNotMatch(notice, /queued for spark/i);
});
