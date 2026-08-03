use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::{agent::AgentDisplayEvent, tools::AgentMode};

pub(crate) const DESKTOP_SERVER_SCHEMA_VERSION: &str = "spark.desktop_server.v1";
const MAX_PROMPT_CHARS: usize = 40_000;

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum DesktopCommand {
    StartRun(DesktopRunRequest),
    CancelRun(CancelRunRequest),
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DesktopRunRequest {
    pub(crate) schema_version: String,
    pub(crate) caller_id: String,
    pub(crate) request_id: String,
    pub(crate) run_id: String,
    pub(crate) prompt: String,
    pub(crate) cwd: String,
    pub(crate) model: String,
    pub(crate) reasoning_effort: String,
    pub(crate) mode: RequestedMode,
    pub(crate) session: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CancelRunRequest {
    schema_version: String,
    caller_id: String,
    request_id: String,
    pub(crate) run_id: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum RequestedMode {
    Ask,
    Work,
}

impl From<RequestedMode> for AgentMode {
    fn from(value: RequestedMode) -> Self {
        match value {
            RequestedMode::Ask => Self::Ask,
            RequestedMode::Work => Self::Work,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DesktopFrame {
    schema_version: &'static str,
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    caller_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sequence: Option<u64>,
    captured_at_unix_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<&'static str>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum RunTerminal {
    Completed,
    Failed,
    Cancelled,
}

impl RunTerminal {
    fn name(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn data(self) -> Value {
        match self {
            Self::Completed => json!({"code": "completed"}),
            Self::Failed => json!({
                "code": "run_failed",
                "message": "Spark could not complete the run."
            }),
            Self::Cancelled => json!({
                "code": "run_cancelled",
                "message": "The run was cancelled."
            }),
        }
    }
}

pub(crate) struct FrameEmitter {
    caller_id: String,
    request_id: String,
    run_id: String,
    sequence: u64,
    snapshot_emitted: bool,
    frames: mpsc::UnboundedSender<DesktopFrame>,
}

impl FrameEmitter {
    pub(crate) fn new(
        request: &DesktopRunRequest,
        frames: mpsc::UnboundedSender<DesktopFrame>,
    ) -> Self {
        Self {
            caller_id: request.caller_id.clone(),
            request_id: request.request_id.clone(),
            run_id: request.run_id.clone(),
            sequence: 0,
            snapshot_emitted: false,
            frames,
        }
    }

    pub(crate) fn snapshot(&mut self, data: Value) {
        self.snapshot_emitted = true;
        self.send("snapshot", None, None, Some(data));
    }

    pub(crate) fn has_snapshot(&self) -> bool {
        self.snapshot_emitted
    }

    pub(crate) fn initialization_error(&mut self) {
        let frame = DesktopFrame {
            schema_version: DESKTOP_SERVER_SCHEMA_VERSION,
            kind: "command_error",
            caller_id: Some(self.caller_id.clone()),
            request_id: Some(self.request_id.clone()),
            run_id: Some(self.run_id.clone()),
            sequence: Some(self.sequence),
            captured_at_unix_ms: captured_at_unix_ms(),
            event: None,
            status: Some("failed"),
            data: None,
            code: Some("run_initialization_failed".to_string()),
            message: Some("Spark could not initialize the run."),
        };
        self.sequence += 1;
        let _ = self.frames.send(frame);
    }

    pub(crate) fn display_events(&mut self, events: Vec<AgentDisplayEvent>) {
        for event in events.into_iter().map(map_display_event) {
            self.send("delta", Some(event.event), None, Some(event.data));
        }
    }

    pub(crate) fn terminal(&mut self, terminal: RunTerminal) {
        self.send(
            "terminal",
            None,
            Some(terminal.name()),
            Some(terminal.data()),
        );
    }

    fn send(
        &mut self,
        kind: &'static str,
        event: Option<String>,
        status: Option<&'static str>,
        data: Option<Value>,
    ) {
        let frame = DesktopFrame {
            schema_version: DESKTOP_SERVER_SCHEMA_VERSION,
            kind,
            caller_id: Some(self.caller_id.clone()),
            request_id: Some(self.request_id.clone()),
            run_id: Some(self.run_id.clone()),
            sequence: Some(self.sequence),
            captured_at_unix_ms: captured_at_unix_ms(),
            event,
            status,
            data,
            code: None,
            message: None,
        };
        self.sequence += 1;
        let _ = self.frames.send(frame);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MappedDisplayEvent {
    pub(crate) event: String,
    pub(crate) data: Value,
}

/// Keep this exhaustive: adding an agent display variant must also add its
/// versioned desktop representation.
pub(crate) fn map_display_event(event: AgentDisplayEvent) -> MappedDisplayEvent {
    match event {
        AgentDisplayEvent::RequestStart { turn, input_chars } => mapped(
            "run.request_started",
            json!({"turn": turn, "input_chars": input_chars}),
        ),
        AgentDisplayEvent::Assistant(text) => mapped("assistant.message", json!({"text": text})),
        AgentDisplayEvent::AssistantDelta(text) => mapped("assistant.delta", json!({"text": text})),
        AgentDisplayEvent::ResponseComplete {
            duration_ms,
            output_tokens,
            time_to_first_token_ms,
            average_tokens_per_second,
        } => mapped(
            "run.response_completed",
            json!({"duration_ms": duration_ms, "output_tokens": output_tokens, "time_to_first_token_ms": time_to_first_token_ms, "average_tokens_per_second": average_tokens_per_second}),
        ),
        AgentDisplayEvent::ReasoningStart => mapped("reasoning.started", json!({})),
        AgentDisplayEvent::ReasoningSummary(text) => {
            mapped("reasoning.summary", json!({"text": text}))
        }
        AgentDisplayEvent::ReasoningFinish => mapped("reasoning.finished", json!({})),
        AgentDisplayEvent::CompactionStart {
            trigger,
            input_chars,
        } => mapped(
            "compaction.started",
            json!({"trigger": trigger, "input_chars": input_chars}),
        ),
        AgentDisplayEvent::CompactionFinish { notice } => {
            mapped("compaction.finished", json!({"notice": notice}))
        }
        AgentDisplayEvent::ToolBatchStart { count } => {
            mapped("tools.batch_started", json!({"count": count}))
        }
        AgentDisplayEvent::ToolCall { name, args } => {
            mapped("tool.called", json!({"name": name, "args": args}))
        }
        AgentDisplayEvent::ToolResult {
            name,
            ok,
            duration_ms,
            output_chars,
            error,
        } => mapped(
            "tool.completed",
            json!({"name": name, "ok": ok, "duration_ms": duration_ms, "output_chars": output_chars, "error": error}),
        ),
        AgentDisplayEvent::ConnectionRetry {
            attempt,
            max_attempts,
            delay_ms,
            error,
        } => mapped(
            "connection.retry",
            json!({"attempt": attempt, "max_attempts": max_attempts, "delay_ms": delay_ms, "error": error}),
        ),
        AgentDisplayEvent::ConnectionRecovered { attempts } => {
            mapped("connection.recovered", json!({"attempts": attempts}))
        }
        AgentDisplayEvent::TransportFallback { from, to, error } => mapped(
            "transport.fallback",
            json!({"from": from, "to": to, "error": error}),
        ),
        AgentDisplayEvent::System(text) => mapped("run.notice", json!({"text": text})),
        AgentDisplayEvent::Warning(text) => mapped("run.warning", json!({"text": text})),
        AgentDisplayEvent::Profile(text) => mapped("run.profile", json!({"text": text})),
    }
}

fn mapped(event: &str, data: Value) -> MappedDisplayEvent {
    MappedDisplayEvent {
        event: event.to_string(),
        data,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ProtocolError {
    caller_id: Option<String>,
    request_id: Option<String>,
    run_id: Option<String>,
    pub(crate) code: String,
    terminal_run: bool,
}

impl ProtocolError {
    pub(crate) fn for_request(request: &DesktopRunRequest, code: &str) -> Self {
        Self {
            caller_id: Some(request.caller_id.clone()),
            request_id: Some(request.request_id.clone()),
            run_id: Some(request.run_id.clone()),
            code: code.to_string(),
            terminal_run: true,
        }
    }

    fn message(&self) -> &'static str {
        match self.code.as_str() {
            "invalid_command" => "The desktop command could not be read.",
            "unsupported_schema" => "This desktop protocol version is not supported.",
            "invalid_identifier" => "The desktop command identifiers are invalid.",
            "invalid_prompt" => "The desktop prompt is invalid.",
            "invalid_workspace" => "The desktop workspace is invalid.",
            "invalid_model" => "The requested model is invalid.",
            "invalid_reasoning_effort" => "The requested reasoning effort is invalid.",
            "invalid_session" => "The requested session is invalid.",
            "run_already_active" => "A desktop run is already active.",
            _ => "The desktop command could not be accepted.",
        }
    }
}

impl DesktopFrame {
    pub(crate) fn protocol_error(error: ProtocolError) -> Self {
        let message = error.message();
        let terminal_run = error.terminal_run;
        Self {
            schema_version: DESKTOP_SERVER_SCHEMA_VERSION,
            kind: "command_error",
            caller_id: error.caller_id,
            request_id: error.request_id,
            run_id: error.run_id,
            sequence: terminal_run.then_some(0),
            captured_at_unix_ms: captured_at_unix_ms(),
            event: None,
            status: terminal_run.then_some("failed"),
            data: None,
            code: Some(error.code),
            message: Some(message),
        }
    }

    pub(crate) fn cancel_ack(request: &CancelRunRequest, cancelled: bool) -> Self {
        Self {
            schema_version: DESKTOP_SERVER_SCHEMA_VERSION,
            kind: "cancel_ack",
            caller_id: Some(request.caller_id.clone()),
            request_id: Some(request.request_id.clone()),
            run_id: Some(request.run_id.clone()),
            sequence: None,
            captured_at_unix_ms: captured_at_unix_ms(),
            event: None,
            status: Some(if cancelled { "accepted" } else { "not_found" }),
            data: None,
            code: None,
            message: None,
        }
    }
}

pub(crate) fn parse_command(line: &str) -> std::result::Result<DesktopCommand, ProtocolError> {
    let command = serde_json::from_str::<DesktopCommand>(line).map_err(|_| ProtocolError {
        caller_id: None,
        request_id: None,
        run_id: None,
        code: "invalid_command".to_string(),
        terminal_run: false,
    })?;
    match &command {
        DesktopCommand::StartRun(request) => validate_run_request(request)?,
        DesktopCommand::CancelRun(request) => validate_cancel_request(request)?,
    }
    Ok(command)
}

fn validate_run_request(request: &DesktopRunRequest) -> std::result::Result<(), ProtocolError> {
    let error = |code| ProtocolError::for_request(request, code);
    if request.schema_version != DESKTOP_SERVER_SCHEMA_VERSION {
        return Err(error("unsupported_schema"));
    }
    if !valid_identifier(&request.caller_id)
        || !valid_identifier(&request.request_id)
        || !valid_identifier(&request.run_id)
    {
        return Err(error("invalid_identifier"));
    }
    if request.prompt.trim().is_empty() || request.prompt.chars().count() > MAX_PROMPT_CHARS {
        return Err(error("invalid_prompt"));
    }
    if request.cwd.trim().is_empty() || request.cwd.len() > 32_000 {
        return Err(error("invalid_workspace"));
    }
    if !valid_identifier(&request.model) {
        return Err(error("invalid_model"));
    }
    if !matches!(
        request.reasoning_effort.as_str(),
        "low" | "medium" | "high" | "xhigh"
    ) {
        return Err(error("invalid_reasoning_effort"));
    }
    if request
        .session
        .as_deref()
        .is_some_and(|session| !crate::config::is_valid_session_name(session))
    {
        return Err(error("invalid_session"));
    }
    Ok(())
}

fn validate_cancel_request(request: &CancelRunRequest) -> std::result::Result<(), ProtocolError> {
    let error = |code: &str| ProtocolError {
        caller_id: Some(request.caller_id.clone()),
        request_id: Some(request.request_id.clone()),
        run_id: Some(request.run_id.clone()),
        code: code.to_string(),
        terminal_run: false,
    };
    if request.schema_version != DESKTOP_SERVER_SCHEMA_VERSION {
        return Err(error("unsupported_schema"));
    }
    if !valid_identifier(&request.caller_id)
        || !valid_identifier(&request.request_id)
        || !valid_identifier(&request.run_id)
    {
        return Err(error("invalid_identifier"));
    }
    Ok(())
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn captured_at_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
impl CancelRunRequest {
    pub(crate) fn test_request(run_id: &str) -> Self {
        Self {
            schema_version: DESKTOP_SERVER_SCHEMA_VERSION.to_string(),
            caller_id: "t3code".to_string(),
            request_id: "request".to_string(),
            run_id: run_id.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versioned_start_run_without_retaining_a_prompt_in_errors() {
        let command = parse_command(r#"{"schema_version":"spark.desktop_server.v1","kind":"start_run","caller_id":"t3code","request_id":"request-1","run_id":"run-1","prompt":"private text","cwd":"C:\\workspace","model":"gpt-5.3-codex-spark","reasoning_effort":"medium","mode":"ask","session":"desktop"}"#).expect("parse command");
        let DesktopCommand::StartRun(request) = command else {
            panic!("expected start request")
        };
        assert_eq!(request.run_id, "run-1");
        assert!(matches!(request.mode, RequestedMode::Ask));
    }

    #[test]
    fn rejects_invalid_schema_and_prompt_without_echoing_input() {
        let error = parse_command(r#"{"schema_version":"wrong","kind":"start_run","caller_id":"t3code","request_id":"request-1","run_id":"run-1","prompt":"private text","cwd":"C:\\workspace","model":"gpt-5.3-codex-spark","reasoning_effort":"medium","mode":"ask"}"#).expect_err("schema must fail");
        assert_eq!(error.code, "unsupported_schema");
        let error = parse_command(r#"{"schema_version":"spark.desktop_server.v1","kind":"start_run","caller_id":"t3code","request_id":"request-1","run_id":"run-1","prompt":" ","cwd":"C:\\workspace","model":"gpt-5.3-codex-spark","reasoning_effort":"medium","mode":"ask"}"#).expect_err("empty prompt must fail");
        assert_eq!(error.code, "invalid_prompt");
        let serialized = serde_json::to_string(&DesktopFrame::protocol_error(error))
            .expect("serialize safe protocol error");
        assert!(!serialized.contains("private text"));
    }

    #[test]
    fn emitter_sequences_snapshot_deltas_and_terminal() {
        let request = DesktopRunRequest {
            schema_version: DESKTOP_SERVER_SCHEMA_VERSION.to_string(),
            caller_id: "t3code".to_string(),
            request_id: "request-1".to_string(),
            run_id: "run-1".to_string(),
            prompt: "private".to_string(),
            cwd: "C:\\workspace".to_string(),
            model: "gpt-5.3-codex-spark".to_string(),
            reasoning_effort: "medium".to_string(),
            mode: RequestedMode::Ask,
            session: None,
        };
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let mut emitter = FrameEmitter::new(&request, sender);
        emitter.snapshot(json!({"kind":"snapshot"}));
        emitter.display_events(vec![AgentDisplayEvent::AssistantDelta("ok".to_string())]);
        emitter.terminal(RunTerminal::Completed);
        let frames = [
            receiver.try_recv().expect("snapshot"),
            receiver.try_recv().expect("delta"),
            receiver.try_recv().expect("terminal"),
        ];
        assert_eq!(frames[0].sequence, Some(0));
        assert_eq!(frames[1].sequence, Some(1));
        assert_eq!(frames[2].sequence, Some(2));
        assert_eq!(frames[2].status, Some("completed"));
        assert_eq!(
            frames[2]
                .data
                .as_ref()
                .and_then(|data| data["code"].as_str()),
            Some("completed")
        );
    }

    #[test]
    fn rejected_start_is_a_sequenced_terminal_frame_but_cancel_is_control_only() {
        let request = DesktopRunRequest {
            schema_version: "wrong".to_string(),
            caller_id: "t3code".to_string(),
            request_id: "request-1".to_string(),
            run_id: "run-1".to_string(),
            prompt: "private".to_string(),
            cwd: "C:\\workspace".to_string(),
            model: "gpt-5.3-codex-spark".to_string(),
            reasoning_effort: "medium".to_string(),
            mode: RequestedMode::Ask,
            session: None,
        };
        let rejected = DesktopFrame::protocol_error(ProtocolError::for_request(
            &request,
            "unsupported_schema",
        ));
        assert_eq!(rejected.sequence, Some(0));
        assert_eq!(rejected.status, Some("failed"));
        assert_eq!(
            rejected.message,
            Some("This desktop protocol version is not supported.")
        );

        let control = DesktopFrame::cancel_ack(&CancelRunRequest::test_request("run-1"), true);
        assert_eq!(control.sequence, None);
        assert_eq!(control.status, Some("accepted"));
    }

    #[test]
    fn initialization_failure_is_the_first_and_only_terminal_stream_frame() {
        let request = DesktopRunRequest {
            schema_version: DESKTOP_SERVER_SCHEMA_VERSION.to_string(),
            caller_id: "t3code".to_string(),
            request_id: "request-1".to_string(),
            run_id: "run-1".to_string(),
            prompt: "private".to_string(),
            cwd: "C:\\workspace".to_string(),
            model: "gpt-5.3-codex-spark".to_string(),
            reasoning_effort: "medium".to_string(),
            mode: RequestedMode::Ask,
            session: None,
        };
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let mut emitter = FrameEmitter::new(&request, sender);
        assert!(!emitter.has_snapshot());
        emitter.initialization_error();
        let frame = receiver.try_recv().expect("initialization error");
        assert_eq!(frame.kind, "command_error");
        assert_eq!(frame.sequence, Some(0));
        assert_eq!(frame.status, Some("failed"));
        assert_eq!(frame.code.as_deref(), Some("run_initialization_failed"));
        assert!(!emitter.has_snapshot());
    }
}
