//! Long-lived, privacy-minimized JSONL bridge for the Proofline desktop host.
//!
//! This module owns the process protocol. It never serializes `AgentSnapshot::input`
//! or writes a raw trace; live activity is deliberately mapped from the curated
//! `AgentDisplayEvent` surface instead.

mod protocol;

use std::time::Duration;

use anyhow::Result;
use protocol::{
    DesktopCommand, DesktopFrame, DesktopRunRequest, FrameEmitter, ProtocolError, RunTerminal,
};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::mpsc,
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

use crate::{
    DEFAULT_COMPACT_AFTER_CHARS, DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS, DEFAULT_MAX_INPUT_CHARS,
    agent::{AgentRunner, take_shared_display_events},
    config, proofline, session,
};

pub(crate) use protocol::parse_command;

const EVENT_DRAIN_INTERVAL: Duration = Duration::from_millis(25);

struct ActiveRun {
    run_id: String,
    cancellation: CancellationToken,
}

struct RunFinished {
    run_id: String,
}

/// Run the desktop protocol until stdin closes. Stdout is owned by exactly one
/// writer task so JSON frames cannot interleave with one another.
pub(crate) async fn run_stdio() -> Result<()> {
    let (frames, frame_receiver) = mpsc::unbounded_channel();
    let writer = tokio::spawn(write_frames(frame_receiver));
    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    let mut runs = JoinSet::new();
    let mut active = None::<ActiveRun>;

    loop {
        tokio::select! {
            line = lines.next_line() => {
                let Some(line) = line? else {
                    if let Some(active) = &active {
                        active.cancellation.cancel();
                    }
                    break;
                };
                if line.trim().is_empty() {
                    continue;
                }
                match parse_command(protocol_line(&line)) {
                    Ok(command) => handle_command(command, &mut active, &mut runs, &frames),
                    Err(error) => send_protocol_error(&frames, error),
                }
            }
            joined = runs.join_next(), if !runs.is_empty() => {
                match joined {
                    Some(Ok(finished)) => {
                        if active.as_ref().is_some_and(|active| active.run_id == finished.run_id) {
                            active = None;
                        }
                    }
                    Some(Err(error)) => eprintln!("desktop server run task failed: {error}"),
                    None => {}
                }
            }
        }
    }

    while let Some(joined) = runs.join_next().await {
        if let Err(error) = joined {
            eprintln!("desktop server run task failed during shutdown: {error}");
        }
    }
    drop(frames);
    writer
        .await
        .map_err(|_| anyhow::anyhow!("desktop server stdout writer task failed"))?
}

fn handle_command(
    command: DesktopCommand,
    active: &mut Option<ActiveRun>,
    runs: &mut JoinSet<RunFinished>,
    frames: &mpsc::UnboundedSender<DesktopFrame>,
) {
    match command {
        DesktopCommand::StartRun(request) => {
            if active.is_some() {
                send_protocol_error(
                    frames,
                    ProtocolError::for_request(&request, "run_already_active"),
                );
                return;
            }
            let cancellation = CancellationToken::new();
            *active = Some(ActiveRun {
                run_id: request.run_id.clone(),
                cancellation: cancellation.clone(),
            });
            let frames = frames.clone();
            runs.spawn(async move {
                let run_id = request.run_id.clone();
                run_desktop_request(request, cancellation, frames).await;
                RunFinished { run_id }
            });
        }
        DesktopCommand::CancelRun(request) => {
            let cancelled = active
                .as_ref()
                .filter(|active| active.run_id == request.run_id)
                .map(|active| {
                    active.cancellation.cancel();
                    true
                })
                .unwrap_or(false);
            let frame = DesktopFrame::cancel_ack(&request, cancelled);
            let _ = frames.send(frame);
        }
    }
}

async fn run_desktop_request(
    request: DesktopRunRequest,
    cancellation: CancellationToken,
    frames: mpsc::UnboundedSender<DesktopFrame>,
) {
    let mut emitter = FrameEmitter::new(&request, frames);
    let result = run_desktop_request_inner(&request, &cancellation, &mut emitter).await;
    if result.is_err() && !emitter.has_snapshot() {
        if let Err(error) = result {
            eprintln!(
                "desktop server run {} failed to initialize: {error:#}",
                request.run_id
            );
        }
        emitter.initialization_error();
        return;
    }
    let terminal = match result {
        Ok(()) => RunTerminal::Completed,
        Err(error) if cancellation.is_cancelled() || error.to_string() == "run cancelled" => {
            RunTerminal::Cancelled
        }
        Err(error) => {
            eprintln!("desktop server run {} failed: {error:#}", request.run_id);
            RunTerminal::Failed
        }
    };
    emitter.terminal(terminal);
}

async fn run_desktop_request_inner(
    request: &DesktopRunRequest,
    cancellation: &CancellationToken,
    emitter: &mut FrameEmitter,
) -> Result<()> {
    let cwd = std::fs::canonicalize(&request.cwd)
        .map_err(|_| anyhow::anyhow!("desktop server workspace does not exist"))?;
    if !cwd.is_dir() {
        anyhow::bail!("desktop server workspace is not a directory");
    }

    let session = session_name_for_request(request);
    session::prepare_default_session_store(Some(&session))?;

    let auth = config::load_auth()?;
    let mut runner = AgentRunner::new_with_reasoning_effort(
        auth,
        cwd,
        request.model.clone(),
        request.reasoning_effort.clone(),
        /* trace */ false,
        /* profile */ false,
        DEFAULT_COMPACT_AFTER_CHARS,
        DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS,
        DEFAULT_MAX_INPUT_CHARS,
        /* interactive */ false,
        Some(session.clone()),
        /* new_session */ false,
        None,
        request.mode.into(),
    )?;

    if !runner.load_session_named(&session)? {
        runner.save_session_named(&session)?;
    }

    let snapshot = proofline::snapshot_default(Some(&session))?;
    emitter.snapshot(serde_json::to_value(snapshot)?);

    let events = runner.use_shared_display();
    let result = {
        let mut run = std::pin::pin!(runner.run_with_cancel(&request.prompt, cancellation.clone()));
        let mut ticker = tokio::time::interval(EVENT_DRAIN_INTERVAL);
        loop {
            tokio::select! {
                result = &mut run => {
                    emitter.display_events(take_shared_display_events(&events));
                    break result;
                }
                _ = ticker.tick() => emitter.display_events(take_shared_display_events(&events)),
            }
        }
    };
    result?;

    runner.save_session_named(&session)?;
    Ok(())
}

fn session_name_for_request(request: &DesktopRunRequest) -> String {
    request
        .session
        .clone()
        .unwrap_or_else(|| derived_session_name(&request.run_id))
}

fn derived_session_name(run_id: &str) -> String {
    let digest = Sha256::digest(run_id.as_bytes());
    let suffix = digest
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("proofline-{suffix}")
}

fn protocol_line(line: &str) -> &str {
    line.trim_start_matches('\u{feff}')
}

fn send_protocol_error(frames: &mpsc::UnboundedSender<DesktopFrame>, error: ProtocolError) {
    eprintln!("desktop server protocol error: {}", error.code);
    let _ = frames.send(DesktopFrame::protocol_error(error));
}

async fn write_frames(mut frames: mpsc::UnboundedReceiver<DesktopFrame>) -> Result<()> {
    let mut stdout = tokio::io::stdout();
    while let Some(frame) = frames.recv().await {
        let body = serde_json::to_vec(&frame)?;
        stdout.write_all(&body).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentDisplayEvent;

    #[test]
    fn maps_every_display_event_to_a_versioned_delta() {
        let events = vec![
            AgentDisplayEvent::RequestStart {
                turn: 1,
                input_chars: 2,
            },
            AgentDisplayEvent::Assistant("answer".into()),
            AgentDisplayEvent::AssistantDelta("delta".into()),
            AgentDisplayEvent::ResponseComplete {
                duration_ms: 1,
                output_tokens: Some(2),
                time_to_first_token_ms: Some(3),
                average_tokens_per_second: Some(4.0),
            },
            AgentDisplayEvent::ReasoningStart,
            AgentDisplayEvent::ReasoningSummary("summary".into()),
            AgentDisplayEvent::ReasoningFinish,
            AgentDisplayEvent::CompactionStart {
                trigger: Some("size".into()),
                input_chars: 5,
            },
            AgentDisplayEvent::CompactionFinish {
                notice: "done".into(),
            },
            AgentDisplayEvent::ToolBatchStart { count: 1 },
            AgentDisplayEvent::ToolCall {
                name: "fs.read".into(),
                args: "{}".into(),
            },
            AgentDisplayEvent::ToolResult {
                name: "fs.read".into(),
                ok: true,
                duration_ms: 1,
                output_chars: 2,
                error: None,
            },
            AgentDisplayEvent::ConnectionRetry {
                attempt: 1,
                max_attempts: 2,
                delay_ms: 3,
                error: "retry".into(),
            },
            AgentDisplayEvent::ConnectionRecovered { attempts: 1 },
            AgentDisplayEvent::TransportFallback {
                from: "WebSocket",
                to: "HTTP/SSE",
                error: "fallback".into(),
            },
            AgentDisplayEvent::System("notice".into()),
            AgentDisplayEvent::Warning("warning".into()),
            AgentDisplayEvent::Profile("profile".into()),
        ];
        let names = events
            .into_iter()
            .map(protocol::map_display_event)
            .map(|event| event.event)
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "run.request_started",
                "assistant.message",
                "assistant.delta",
                "run.response_completed",
                "reasoning.started",
                "reasoning.summary",
                "reasoning.finished",
                "compaction.started",
                "compaction.finished",
                "tools.batch_started",
                "tool.called",
                "tool.completed",
                "connection.retry",
                "connection.recovered",
                "transport.fallback",
                "run.notice",
                "run.warning",
                "run.profile",
            ]
        );
    }

    #[test]
    fn cancellation_only_accepts_the_matching_active_run() {
        let cancellation = CancellationToken::new();
        let mut active = Some(ActiveRun {
            run_id: "run-a".into(),
            cancellation: cancellation.clone(),
        });
        let request = protocol::CancelRunRequest::test_request("run-b");
        let (frames, _receiver) = mpsc::unbounded_channel();
        handle_command(
            DesktopCommand::CancelRun(request),
            &mut active,
            &mut JoinSet::new(),
            &frames,
        );
        assert!(!cancellation.is_cancelled());
        let request = protocol::CancelRunRequest::test_request("run-a");
        handle_command(
            DesktopCommand::CancelRun(request),
            &mut active,
            &mut JoinSet::new(),
            &frames,
        );
        assert!(cancellation.is_cancelled());
    }

    #[test]
    fn derives_a_stable_valid_session_when_the_caller_omits_one() {
        let first = derived_session_name("run-1");
        assert_eq!(first, derived_session_name("run-1"));
        assert_ne!(first, derived_session_name("run-2"));
        assert!(crate::config::is_valid_session_name(&first));
    }

    #[test]
    fn accepts_a_utf8_bom_on_the_first_protocol_line() {
        let command = protocol_line(
            "\u{feff}{\"schema_version\":\"spark.desktop_server.v1\",\"kind\":\"cancel_run\",\"caller_id\":\"proofline\",\"request_id\":\"request-1\",\"run_id\":\"run-1\"}",
        );
        assert!(parse_command(command).is_ok());
    }
}
