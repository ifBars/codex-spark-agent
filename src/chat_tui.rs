use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use serde_json::Value;

use crate::agent::{
    AgentDisplayEvent, AgentRunner, SharedDisplayEvents, take_shared_display_events,
};
use crate::{chat, sessions, skill_commands, tools};

mod markdown;

use markdown::render_markdown_lines;

pub(crate) async fn run(
    runner: &mut AgentRunner,
    session_name: Option<String>,
    cwd: PathBuf,
) -> Result<()> {
    runner.use_buffered_display();
    let mut terminal = TuiTerminal::enter()?;
    let mut app = ChatTui::new(session_name, cwd);
    app.push_system("Type /help for commands, /exit to quit.");
    let result = app.run(runner, &mut terminal.terminal).await;
    terminal.exit()?;
    result
}

struct TuiTerminal {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TuiTerminal {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let terminal = Terminal::new(CrosstermBackend::new(stdout))?;
        Ok(Self { terminal })
    }

    fn exit(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

impl Drop for TuiTerminal {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

struct ChatTui {
    session_name: Option<String>,
    cwd: PathBuf,
    messages: Vec<TranscriptMessage>,
    input: String,
    scroll_back: u16,
    running: bool,
    should_quit: bool,
    activity: ActivityState,
    tool_group_index: Option<usize>,
    compaction_index: Option<usize>,
    tool_batch_seq: usize,
    tool_call_seq: usize,
    activity_details_expanded: bool,
}

#[derive(Clone)]
struct TranscriptMessage {
    role: MessageRole,
    body: String,
}

#[derive(Clone, Copy)]
enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
    Compaction,
    Warning,
    Profile,
}

#[derive(Clone, Copy, Default)]
enum ActivityPhase {
    #[default]
    Idle,
    Thinking,
    Receiving,
    Compacting,
    Tools,
}

#[derive(Default)]
struct ActivityState {
    phase: ActivityPhase,
    detail: String,
    turn: Option<usize>,
    input_chars: Option<usize>,
    total_tools: usize,
    finished_tools: usize,
    current_tool: Option<String>,
    tick: usize,
}

impl ChatTui {
    fn new(session_name: Option<String>, cwd: PathBuf) -> Self {
        Self {
            session_name,
            cwd,
            messages: Vec::new(),
            input: String::new(),
            scroll_back: 0,
            running: false,
            should_quit: false,
            activity: ActivityState::default(),
            tool_group_index: None,
            compaction_index: None,
            tool_batch_seq: 0,
            tool_call_seq: 0,
            activity_details_expanded: false,
        }
    }

    async fn run(
        &mut self,
        runner: &mut AgentRunner,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| self.draw(frame))?;
            self.tick_activity();

            if !event::poll(Duration::from_millis(100))? {
                continue;
            }

            let Event::Key(key) = event::read()? else {
                continue;
            };
            if !should_handle_key_event(key) {
                continue;
            }
            self.handle_key(runner, terminal, key).await?;
            self.drain_agent_events(runner);
        }
        Ok(())
    }

    async fn handle_key(
        &mut self,
        runner: &mut AgentRunner,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        key: KeyEvent,
    ) -> Result<()> {
        if is_toggle_activity_details_key(key) {
            self.toggle_activity_details();
            return Ok(());
        }

        if self.running {
            return Ok(());
        }

        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char(ch) => self.input.push(ch),
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Esc => self.input.clear(),
            KeyCode::Enter => {
                let input = self.input.trim().trim_start_matches('\u{feff}').to_string();
                self.input.clear();
                if !input.is_empty() {
                    self.submit(runner, terminal, input).await?;
                }
            }
            KeyCode::PageUp => self.scroll_by(-8),
            KeyCode::PageDown => self.scroll_by(8),
            KeyCode::Up => self.scroll_by(-1),
            KeyCode::Down => self.scroll_by(1),
            _ => {}
        }
        Ok(())
    }

    async fn submit(
        &mut self,
        runner: &mut AgentRunner,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        input: String,
    ) -> Result<()> {
        if self.handle_command(runner, &input).await? {
            return Ok(());
        }

        self.push_user(&input);
        self.running = true;
        terminal.draw(|frame| self.draw(frame))?;
        let events = runner.use_shared_display();

        let run_result = if let Err(error) =
            skill_commands::load_skill_mentions(runner, &self.cwd, &input).await
        {
            Err(error)
        } else {
            self.run_agent_with_redraw(runner, terminal, &events, &input)
                .await
        };

        self.running = false;
        self.drain_shared_agent_events(&events);
        self.activity.finish();
        self.tool_group_index = None;
        if let Err(error) = run_result {
            self.push_warning(format!("error: {error:#}"));
        }
        if let Some(name) = &self.session_name {
            runner.save_session_named(name)?;
        }
        Ok(())
    }

    async fn run_agent_with_redraw(
        &mut self,
        runner: &mut AgentRunner,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        events: &SharedDisplayEvents,
        input: &str,
    ) -> Result<()> {
        let mut run = std::pin::pin!(runner.run(input));
        loop {
            tokio::select! {
                result = &mut run => {
                    self.drain_shared_agent_events(events);
                    terminal.draw(|frame| self.draw(frame))?;
                    return result;
                }
                _ = tokio::time::sleep(Duration::from_millis(50)) => {
                    self.drain_shared_agent_events(events);
                    self.handle_running_input()?;
                    self.tick_activity();
                    terminal.draw(|frame| self.draw(frame))?;
                }
            }
        }
    }

    fn handle_running_input(&mut self) -> Result<()> {
        while event::poll(Duration::from_millis(0))? {
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if !should_handle_key_event(key) {
                continue;
            }
            match key.code {
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.should_quit = true;
                }
                _ if is_toggle_activity_details_key(key) => self.toggle_activity_details(),
                KeyCode::PageUp => self.scroll_by(-8),
                KeyCode::PageDown => self.scroll_by(8),
                KeyCode::Up => self.scroll_by(-1),
                KeyCode::Down => self.scroll_by(1),
                _ => {}
            }
        }
        Ok(())
    }

    async fn handle_command(&mut self, runner: &mut AgentRunner, input: &str) -> Result<bool> {
        if let Some(command) = chat::command_args(input, "/session") {
            sessions::handle_session_command(runner, &mut self.session_name, command.trim())?;
            return Ok(true);
        }
        if let Some(command) = chat::command_args(input, "/new") {
            sessions::handle_new_session_command(runner, &mut self.session_name, command.trim())?;
            return Ok(true);
        }
        if input == "/skills" {
            skill_commands::handle_skill_command(runner, &self.cwd, "list").await?;
            return Ok(true);
        }
        if let Some(command) = chat::command_args(input, "/skill") {
            skill_commands::handle_skill_command(runner, &self.cwd, command.trim()).await?;
            if let Some(name) = &self.session_name {
                runner.save_session_named(name)?;
            }
            return Ok(true);
        }
        if let Some(command) = chat::command_args(input, "/mode") {
            match chat::parse_mode(command.trim()) {
                Some(mode) => {
                    runner.set_mode(mode);
                    if let Some(name) = &self.session_name {
                        runner.save_session_named(name)?;
                    }
                    self.push_system(format!("mode: {}", mode.name()));
                }
                None => self.push_warning("usage: /mode ask|work"),
            }
            return Ok(true);
        }

        match input {
            "/exit" | "/quit" => {
                self.should_quit = true;
                Ok(true)
            }
            "/help" => {
                self.push_system(
                    "Commands: /help, /status, /mode, /ask, /work, /profile, /compact, /session, /new, /skill, /skills, /save, /clear, /exit\n\
Session commands: /session, /session list, /session open <name>, /session new <name>, /session use <name>, /session rename [old] <new>, /session delete <name>\n\
Navigation: PageUp/PageDown or Up/Down scrolls, Ctrl+T toggles tool details, Esc clears the composer, Ctrl+C exits.",
                );
                Ok(true)
            }
            "/status" => {
                self.push_system(format!(
                    "conversation input JSON chars: {}\n{}",
                    runner.input_chars()?,
                    runner.profile_status()
                ));
                Ok(true)
            }
            "/mode" => {
                self.push_system(format!("mode: {}", runner.mode().name()));
                Ok(true)
            }
            "/ask" => {
                runner.set_mode(tools::AgentMode::Ask);
                if let Some(name) = &self.session_name {
                    runner.save_session_named(name)?;
                }
                self.push_system("mode: ask");
                Ok(true)
            }
            "/work" => {
                runner.set_mode(tools::AgentMode::Work);
                if let Some(name) = &self.session_name {
                    runner.save_session_named(name)?;
                }
                self.push_system("mode: work");
                Ok(true)
            }
            "/profile" => {
                self.push_profile(serde_json::to_string_pretty(&runner.profile_summary())?);
                Ok(true)
            }
            "/compact" => {
                match runner.compact_now().await {
                    Ok(Some(report)) => {
                        self.push_system(serde_json::to_string_pretty(&report)?);
                        self.push_system(format!(
                            "conversation input JSON chars: {}",
                            runner.input_chars()?
                        ));
                        if let Some(name) = &self.session_name {
                            runner.save_session_named(name)?;
                        }
                    }
                    Ok(None) => self.push_system("nothing to compact"),
                    Err(error) => self.push_warning(format!("error: {error:#}")),
                }
                Ok(true)
            }
            "/clear" => {
                runner.clear_conversation();
                self.messages.clear();
                if let Some(name) = &self.session_name {
                    runner.save_session_named(name)?;
                }
                self.push_system("conversation cleared");
                Ok(true)
            }
            "/save" => {
                if let Some(name) = &self.session_name {
                    runner.save_session_named(name)?;
                    self.push_system(format!("saved session: {name}"));
                } else {
                    self.push_system("no session configured; start with --session <name>");
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn drain_agent_events(&mut self, runner: &mut AgentRunner) {
        self.apply_agent_events(runner.take_display_events());
    }

    fn drain_shared_agent_events(&mut self, events: &SharedDisplayEvents) {
        self.apply_agent_events(take_shared_display_events(events));
    }

    fn apply_agent_events(&mut self, events: Vec<AgentDisplayEvent>) {
        for event in events {
            match event {
                AgentDisplayEvent::RequestStart { turn, input_chars } => {
                    self.activity.start_request(turn, input_chars);
                }
                AgentDisplayEvent::Assistant(text) => {
                    self.activity.receive_response();
                    self.tool_group_index = None;
                    self.push_assistant(text);
                }
                AgentDisplayEvent::AssistantDelta(text) => {
                    self.activity.receive_response();
                    self.tool_group_index = None;
                    self.push_assistant_delta(&text);
                }
                AgentDisplayEvent::CompactionStart {
                    trigger,
                    input_chars,
                } => {
                    self.tool_group_index = None;
                    self.activity
                        .start_compaction(trigger.as_deref(), input_chars);
                    self.start_compaction_message(format!(
                        "running trigger={} input={} chars",
                        trigger.as_deref().unwrap_or("manual"),
                        input_chars
                    ));
                }
                AgentDisplayEvent::CompactionFinish { notice } => {
                    self.activity.finish_compaction();
                    self.finish_compaction_message(notice);
                }
                AgentDisplayEvent::ToolBatchStart { count } => {
                    self.activity.start_tools(count);
                    self.start_tool_group(count);
                }
                AgentDisplayEvent::ToolCall { name, args } => {
                    self.activity.start_tool(&name);
                    self.append_tool_call(&name, &args);
                }
                AgentDisplayEvent::ToolResult {
                    name,
                    ok,
                    duration_ms,
                    output_chars,
                    error,
                } => {
                    self.activity.finish_tool(&name);
                    self.append_tool_result(&name, ok, duration_ms, output_chars, error.as_deref());
                }
                AgentDisplayEvent::System(text) => {
                    self.tool_group_index = None;
                    self.push_system(text);
                }
                AgentDisplayEvent::Warning(text) => {
                    self.tool_group_index = None;
                    self.push_warning(text);
                }
                AgentDisplayEvent::Profile(text) => {
                    self.tool_group_index = None;
                    self.push_profile(text);
                }
            }
        }
    }

    fn draw(&self, frame: &mut ratatui::Frame<'_>) {
        let [header, transcript, composer, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .areas(frame.area());

        let session = self
            .session_name
            .as_ref()
            .cloned()
            .unwrap_or_else(|| "no session".to_string());
        let state = if self.running { "running" } else { "ready" };
        let header_line = Line::from(vec![
            Span::styled(" Spark ", Style::new().black().on_cyan().bold()),
            Span::raw(" "),
            Span::styled(state, Style::new().fg(Color::Cyan)),
            Span::raw(" "),
            Span::styled(self.activity.header_label(), self.activity.header_style()),
            Span::raw("  "),
            Span::styled(session, Style::new().fg(Color::DarkGray)),
        ]);
        frame.render_widget(header_line, header);

        let transcript_lines = self.render_transcript_lines();
        let transcript_height = transcript.height.saturating_sub(2) as usize;
        let transcript_width = transcript.width.saturating_sub(2) as usize;
        let tail_scroll = wrapped_line_count(&transcript_lines, transcript_width)
            .saturating_sub(transcript_height) as u16;
        let scroll = tail_scroll.saturating_sub(self.scroll_back);
        let transcript_text = Text::from(transcript_lines);
        let transcript_widget = Paragraph::new(transcript_text)
            .block(Block::new().borders(Borders::ALL).title("Conversation"))
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0));
        frame.render_widget(transcript_widget, transcript);

        let composer_text = if self.input.is_empty() {
            Text::from(Line::from(Span::styled(
                "Ask Spark...",
                Style::new().fg(Color::DarkGray),
            )))
        } else {
            Text::from(self.input.as_str())
        };
        let composer_widget = Paragraph::new(composer_text)
            .block(Block::new().borders(Borders::ALL).title("Message"))
            .wrap(Wrap { trim: false });
        frame.render_widget(composer_widget, composer);

        let footer_line = if self.running {
            Line::from(vec![
                Span::styled("PgUp/PgDn", Style::new().fg(Color::Cyan)),
                Span::raw(" scroll  "),
                Span::styled("Ctrl+C", Style::new().fg(Color::Cyan)),
                Span::raw(" exit  "),
                Span::styled("Ctrl+T", Style::new().fg(Color::Cyan)),
                Span::raw(if self.activity_details_expanded {
                    " collapse tools  "
                } else {
                    " expand tools  "
                }),
                Span::styled("Spark is busy", Style::new().fg(Color::DarkGray)),
            ])
        } else {
            Line::from(vec![
                Span::styled("Enter", Style::new().fg(Color::Cyan)),
                Span::raw(" send  "),
                Span::styled("Esc", Style::new().fg(Color::Cyan)),
                Span::raw(" clear  "),
                Span::styled("PgUp/PgDn", Style::new().fg(Color::Cyan)),
                Span::raw(" scroll  "),
                Span::styled("Ctrl+T", Style::new().fg(Color::Cyan)),
                Span::raw(if self.activity_details_expanded {
                    " collapse  "
                } else {
                    " expand  "
                }),
                Span::styled("/help", Style::new().fg(Color::Cyan)),
                Span::raw(" commands"),
            ])
        };
        frame.render_widget(footer_line, footer);
    }

    fn render_transcript_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        for (index, message) in self.messages.iter().enumerate() {
            lines.push(role_line(message.role));
            let body_lines = match message.role {
                MessageRole::Assistant | MessageRole::User => render_markdown_lines(&message.body),
                MessageRole::Tool => render_tool_lines(
                    &message.body,
                    self.activity_details_expanded,
                    self.tool_group_index == Some(index)
                        && matches!(self.activity.phase, ActivityPhase::Tools),
                    activity_row_spinner(self.activity.tick),
                ),
                MessageRole::Compaction => render_compaction_lines(
                    &message.body,
                    self.activity_details_expanded,
                    self.compaction_index == Some(index)
                        && matches!(self.activity.phase, ActivityPhase::Compacting),
                    activity_row_spinner(self.activity.tick),
                ),
                MessageRole::System | MessageRole::Warning | MessageRole::Profile => {
                    plain_lines(&message.body)
                }
            };
            for line in body_lines {
                lines.push(indent_line(line));
            }
            lines.push(Line::from(""));
        }
        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "No messages yet.",
                Style::new().fg(Color::DarkGray),
            )));
        }
        lines
    }

    fn push_user(&mut self, body: impl Into<String>) {
        self.push_message(MessageRole::User, body);
    }

    fn push_assistant(&mut self, body: impl Into<String>) {
        self.push_message(MessageRole::Assistant, body);
    }

    fn push_assistant_delta(&mut self, delta: &str) {
        if delta.is_empty() {
            return;
        }
        if let Some(last) = self.messages.last_mut()
            && matches!(last.role, MessageRole::Assistant)
        {
            last.body.push_str(delta);
            self.scroll_back = 0;
            return;
        }
        self.push_message(MessageRole::Assistant, delta.to_string());
    }

    fn push_system(&mut self, body: impl Into<String>) {
        self.push_message(MessageRole::System, body);
    }

    fn push_tool(&mut self, body: impl Into<String>) {
        self.push_message(MessageRole::Tool, body);
    }

    fn start_tool_group(&mut self, count: usize) {
        self.tool_batch_seq = self.tool_batch_seq.saturating_add(1);
        self.tool_call_seq = 0;
        let line = format!(
            "batch {}: {count} call{}",
            self.tool_batch_seq,
            plural_s(count)
        );
        let index = if self
            .messages
            .last()
            .is_some_and(|message| matches!(message.role, MessageRole::Tool))
        {
            let index = self.messages.len().saturating_sub(1);
            if let Some(message) = self.messages.get_mut(index) {
                if !message.body.trim().is_empty() {
                    message.body.push('\n');
                    message.body.push('\n');
                }
                message.body.push_str(&line);
            }
            index
        } else {
            self.push_tool(line);
            self.messages.len().saturating_sub(1)
        };
        self.tool_group_index = Some(index);
    }

    fn append_tool_call(&mut self, name: &str, args: &str) {
        self.tool_call_seq = self.tool_call_seq.saturating_add(1);
        self.append_tool_group_line(format!(
            "  [{}] {name} {}",
            self.tool_call_seq,
            format_tool_args(name, args)
        ));
    }

    fn append_tool_result(
        &mut self,
        name: &str,
        ok: bool,
        duration_ms: u64,
        output_chars: usize,
        error: Option<&str>,
    ) {
        let status = if ok { "ok" } else { "failed" };
        let error = error
            .filter(|error| !error.trim().is_empty())
            .map(|error| format!(": {}", compact_inline(error, 160)))
            .unwrap_or_default();
        self.append_tool_group_line(format!(
            "      {status} {name} {duration_ms}ms {}{error}",
            format_chars(output_chars)
        ));
    }

    fn append_tool_group_line(&mut self, line: String) {
        let index = match self.tool_group_index {
            Some(index)
                if self
                    .messages
                    .get(index)
                    .is_some_and(|message| matches!(message.role, MessageRole::Tool)) =>
            {
                index
            }
            _ => {
                self.push_tool("Tool calls");
                let index = self.messages.len().saturating_sub(1);
                self.tool_group_index = Some(index);
                index
            }
        };
        if let Some(message) = self.messages.get_mut(index) {
            message.body.push('\n');
            message.body.push_str(&line);
            self.scroll_back = 0;
        }
    }

    fn push_warning(&mut self, body: impl Into<String>) {
        self.push_message(MessageRole::Warning, body);
    }

    fn push_profile(&mut self, body: impl Into<String>) {
        self.push_message(MessageRole::Profile, body);
    }

    fn start_compaction_message(&mut self, body: String) {
        self.push_message(MessageRole::Compaction, body);
        self.compaction_index = self.messages.len().checked_sub(1);
    }

    fn finish_compaction_message(&mut self, notice: String) {
        let line = format!("done {notice}");
        let index = match self.compaction_index {
            Some(index)
                if self
                    .messages
                    .get(index)
                    .is_some_and(|message| matches!(message.role, MessageRole::Compaction)) =>
            {
                index
            }
            _ => {
                self.push_message(MessageRole::Compaction, "");
                self.messages.len().saturating_sub(1)
            }
        };
        if let Some(message) = self.messages.get_mut(index) {
            if !message.body.trim().is_empty() {
                message.body.push('\n');
            }
            message.body.push_str(&line);
            self.scroll_back = 0;
        }
        self.compaction_index = None;
    }

    fn push_message(&mut self, role: MessageRole, body: impl Into<String>) {
        self.messages.push(TranscriptMessage {
            role,
            body: body.into(),
        });
        self.scroll_back = 0;
    }

    fn scroll_by(&mut self, delta: i16) {
        if delta < 0 {
            self.scroll_back = self.scroll_back.saturating_add(delta.unsigned_abs());
        } else {
            self.scroll_back = self.scroll_back.saturating_sub(delta as u16);
        }
    }

    fn tick_activity(&mut self) {
        if self.running {
            self.activity.tick = self.activity.tick.wrapping_add(1);
        }
    }

    fn toggle_activity_details(&mut self) {
        self.activity_details_expanded = !self.activity_details_expanded;
        self.scroll_back = 0;
    }
}

impl ActivityState {
    fn start_request(&mut self, turn: usize, input_chars: usize) {
        self.phase = ActivityPhase::Thinking;
        self.turn = Some(turn);
        self.input_chars = Some(input_chars);
        self.detail = format!("request #{turn}, {input_chars} chars");
        self.current_tool = None;
    }

    fn receive_response(&mut self) {
        self.phase = ActivityPhase::Receiving;
        self.detail = "streaming response".to_string();
        self.current_tool = None;
    }

    fn start_compaction(&mut self, trigger: Option<&str>, input_chars: usize) {
        self.phase = ActivityPhase::Compacting;
        self.input_chars = Some(input_chars);
        self.detail = format!(
            "trigger={}, input={} chars",
            trigger.unwrap_or("manual"),
            input_chars
        );
        self.current_tool = None;
    }

    fn finish_compaction(&mut self) {
        if !matches!(self.phase, ActivityPhase::Tools) {
            self.phase = ActivityPhase::Thinking;
            self.detail = "compaction complete".to_string();
        }
    }

    fn start_tools(&mut self, count: usize) {
        self.phase = ActivityPhase::Tools;
        self.total_tools = count;
        self.finished_tools = 0;
        self.current_tool = None;
        self.detail = format!("0/{count} tools");
    }

    fn start_tool(&mut self, name: &str) {
        self.phase = ActivityPhase::Tools;
        self.current_tool = Some(name.to_string());
        self.detail = if self.total_tools == 0 {
            format!("running {name}")
        } else {
            format!(
                "{}/{} tools, running {name}",
                self.finished_tools, self.total_tools
            )
        };
    }

    fn finish_tool(&mut self, name: &str) {
        self.phase = ActivityPhase::Tools;
        self.finished_tools = self.finished_tools.saturating_add(1);
        self.current_tool = Some(name.to_string());
        self.detail = if self.total_tools == 0 {
            format!("finished {name}")
        } else {
            format!(
                "{}/{} tools complete",
                self.finished_tools, self.total_tools
            )
        };
    }

    fn finish(&mut self) {
        self.phase = ActivityPhase::Idle;
        self.detail.clear();
        self.current_tool = None;
        self.total_tools = 0;
        self.finished_tools = 0;
    }

    fn header_label(&self) -> String {
        match self.phase {
            ActivityPhase::Idle => "idle".to_string(),
            ActivityPhase::Thinking => format!("{} thinking {}", self.spinner(), self.detail),
            ActivityPhase::Receiving => format!("{} receiving {}", self.spinner(), self.detail),
            ActivityPhase::Compacting => {
                format!("{} compacting {}", self.spinner(), self.detail)
            }
            ActivityPhase::Tools => format!("{} tools {}", self.spinner(), self.detail),
        }
    }

    fn header_style(&self) -> Style {
        match self.phase {
            ActivityPhase::Idle => Style::new().fg(Color::DarkGray),
            ActivityPhase::Thinking | ActivityPhase::Receiving => Style::new().fg(Color::Cyan),
            ActivityPhase::Compacting => Style::new().fg(Color::Yellow),
            ActivityPhase::Tools => Style::new().fg(Color::Magenta),
        }
    }

    fn spinner(&self) -> &'static str {
        if matches!(self.phase, ActivityPhase::Idle) {
            return "";
        }
        const FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
        FRAMES[self.tick % FRAMES.len()]
    }
}

fn compact_inline(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    let keep = max_chars.saturating_sub(3);
    let mut trimmed = normalized.chars().take(keep).collect::<String>();
    trimmed.push_str("...");
    trimmed
}

fn role_line(role: MessageRole) -> Line<'static> {
    let (label, color) = match role {
        MessageRole::User => ("You", Color::Green),
        MessageRole::Assistant => ("Spark", Color::Cyan),
        MessageRole::System => ("System", Color::Yellow),
        MessageRole::Tool => ("Tools", Color::Magenta),
        MessageRole::Compaction => ("Compaction", Color::Yellow),
        MessageRole::Warning => ("Warning", Color::Red),
        MessageRole::Profile => ("Profile", Color::Blue),
    };
    Line::from(Span::styled(
        label.to_string(),
        Style::new().fg(color).add_modifier(Modifier::BOLD),
    ))
}

fn render_tool_lines(
    text: &str,
    expanded: bool,
    active: bool,
    spinner: &'static str,
) -> Vec<Line<'static>> {
    let summary = summarize_tool_text(text);
    let mut lines = vec![render_tool_summary_line(
        &summary, expanded, active, spinner,
    )];
    if !expanded {
        return lines;
    }

    lines.extend(text.lines().map(|line| {
        if line.starts_with("batch ") {
            return Line::from(Span::styled(
                line.to_string(),
                Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ));
        }
        if let Some(rest) = line.strip_prefix("  [") {
            let Some((index, after_index)) = rest.split_once("] ") else {
                return Line::from(Span::raw(line.to_string()));
            };
            let Some((tool, summary)) = after_index.split_once(' ') else {
                return Line::from(Span::raw(line.to_string()));
            };
            return Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("[{index}] "), Style::new().fg(Color::DarkGray)),
                Span::styled(
                    tool.to_string(),
                    Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(summary.to_string(), Style::new().fg(Color::Gray)),
            ]);
        }
        if let Some(rest) = line.strip_prefix("      ") {
            let ok = rest.starts_with("ok ");
            let failed = rest.starts_with("failed ");
            if ok || failed {
                let status = if ok { "ok" } else { "failed" };
                let color = if ok { Color::Green } else { Color::Red };
                let suffix = rest.strip_prefix(status).unwrap_or(rest);
                return Line::from(vec![
                    Span::raw("      "),
                    Span::styled(
                        status.to_string(),
                        Style::new().fg(color).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(suffix.to_string(), Style::new().fg(Color::DarkGray)),
                ]);
            }
        }
        Line::from(Span::raw(line.to_string()))
    }));
    lines
}

fn render_compaction_lines(
    text: &str,
    expanded: bool,
    active: bool,
    spinner: &'static str,
) -> Vec<Line<'static>> {
    let summary = summarize_compaction_text(text);
    let marker = if expanded { "v" } else { ">" };
    let state = if active {
        spinner
    } else if summary.done {
        "ok"
    } else {
        ".."
    };
    let mut lines = vec![Line::from(vec![
        Span::styled(format!("{marker} "), Style::new().fg(Color::DarkGray)),
        Span::styled(
            state.to_string(),
            Style::new()
                .fg(if summary.done {
                    Color::Green
                } else {
                    Color::Yellow
                })
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(summary.text, Style::new().fg(Color::Yellow)),
        Span::styled(
            if expanded {
                "  Ctrl+T collapse"
            } else {
                "  Ctrl+T expand"
            },
            Style::new().fg(Color::DarkGray),
        ),
    ])];

    if !expanded {
        return lines;
    }

    lines.extend(text.lines().map(|line| {
        let color = if line.starts_with("done ") {
            Color::Green
        } else {
            Color::Yellow
        };
        Line::from(Span::styled(
            line.to_string(),
            Style::new().fg(color).add_modifier(Modifier::BOLD),
        ))
    }));
    lines
}

#[derive(Default)]
struct ToolSummary {
    batches: usize,
    calls: usize,
    ok: usize,
    failed: usize,
    tool_counts: BTreeMap<String, usize>,
    last_call: Option<String>,
}

struct CompactionSummary {
    text: String,
    done: bool,
}

fn render_tool_summary_line(
    summary: &ToolSummary,
    expanded: bool,
    active: bool,
    spinner: &'static str,
) -> Line<'static> {
    let marker = if expanded { "v" } else { ">" };
    let state = if active {
        spinner.to_string()
    } else if summary.failed > 0 {
        format!("{} failed", summary.failed)
    } else {
        "ok".to_string()
    };
    let status_color = if active {
        Color::Yellow
    } else if summary.failed > 0 {
        Color::Red
    } else {
        Color::Green
    };
    let tools = summary
        .tool_counts
        .iter()
        .map(|(tool, count)| format!("{tool} x{count}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut details = format!(
        "{} batch{}, {} call{}, {} ok",
        summary.batches,
        plural_s(summary.batches),
        summary.calls,
        plural_s(summary.calls),
        summary.ok
    );
    if summary.failed > 0 {
        details.push_str(&format!(", {} failed", summary.failed));
    }
    if !tools.is_empty() {
        details.push_str(&format!(" - {tools}"));
    }
    if active && let Some(last_call) = &summary.last_call {
        details.push_str(&format!(" - running {last_call}"));
    }

    Line::from(vec![
        Span::styled(format!("{marker} "), Style::new().fg(Color::DarkGray)),
        Span::styled(
            state,
            Style::new().fg(status_color).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(details, Style::new().fg(Color::Gray)),
        Span::styled(
            if expanded {
                "  Ctrl+T collapse"
            } else {
                "  Ctrl+T expand"
            },
            Style::new().fg(Color::DarkGray),
        ),
    ])
}

fn summarize_tool_text(text: &str) -> ToolSummary {
    let mut summary = ToolSummary::default();
    for line in text.lines() {
        if line.starts_with("batch ") {
            summary.batches += 1;
            continue;
        }
        if let Some(rest) = line.strip_prefix("  [") {
            let Some((_, after_index)) = rest.split_once("] ") else {
                continue;
            };
            let Some((tool, detail)) = after_index.split_once(' ') else {
                continue;
            };
            summary.calls += 1;
            *summary.tool_counts.entry(tool.to_string()).or_default() += 1;
            summary.last_call = Some(format!("{tool} {}", compact_inline(detail, 80)));
            continue;
        }
        if let Some(rest) = line.strip_prefix("      ") {
            if rest.starts_with("ok ") {
                summary.ok += 1;
            } else if rest.starts_with("failed ") {
                summary.failed += 1;
            }
        }
    }
    summary
}

fn summarize_compaction_text(text: &str) -> CompactionSummary {
    let done_line = text.lines().rev().find(|line| line.starts_with("done "));
    if let Some(done) = done_line {
        return CompactionSummary {
            text: done.trim_start_matches("done ").to_string(),
            done: true,
        };
    }
    let text = text
        .lines()
        .next()
        .filter(|line| !line.trim().is_empty())
        .unwrap_or("compaction running")
        .to_string();
    CompactionSummary { text, done: false }
}

fn activity_row_spinner(tick: usize) -> &'static str {
    const FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
    FRAMES[tick % FRAMES.len()]
}

fn plain_lines(text: &str) -> Vec<Line<'static>> {
    text.lines()
        .map(|line| Line::from(Span::raw(line.to_string())))
        .collect()
}

fn format_tool_args(name: &str, args: &str) -> String {
    let Ok(value) = serde_json::from_str::<Value>(args) else {
        return compact_inline(args, 180);
    };
    let Some(object) = value.as_object() else {
        return compact_inline(args, 180);
    };

    match name {
        "fs.read" => {
            let path = string_field(object, "path").unwrap_or("<missing path>");
            let offset = number_field(object, "offset");
            let limit = number_field(object, "limit");
            match (offset, limit) {
                (Some(offset), Some(limit)) => format!("{path} lines {offset}..+{limit}"),
                (Some(offset), None) => format!("{path} from line {offset}"),
                (None, Some(limit)) => format!("{path} first {limit} lines"),
                (None, None) => path.to_string(),
            }
        }
        "fs.search" => {
            let query = string_field(object, "query")
                .or_else(|| string_field(object, "pattern"))
                .unwrap_or("<missing query>");
            let path = string_field(object, "path").unwrap_or(".");
            let label = if object
                .get("regex")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                "regex"
            } else {
                "query"
            };
            format!("{label} {query:?} in {path}")
        }
        "fs.stat" => string_field(object, "path")
            .map(str::to_string)
            .unwrap_or_else(|| compact_inline(args, 180)),
        "fs.write" | "fs.edit" | "fs.replace" | "fs.rename" => object
            .get("path")
            .and_then(Value::as_str)
            .or_else(|| object.get("from").and_then(Value::as_str))
            .map(|path| {
                let extra = object
                    .get("to")
                    .and_then(Value::as_str)
                    .map(|to| format!(" -> {to}"))
                    .unwrap_or_default();
                format!("{path}{extra}")
            })
            .unwrap_or_else(|| compact_inline(args, 180)),
        "cmd.exec" => string_field(object, "command")
            .map(|command| compact_inline(command, 180))
            .unwrap_or_else(|| compact_inline(args, 180)),
        _ => compact_inline(args, 180),
    }
}

fn string_field<'a>(object: &'a serde_json::Map<String, Value>, key: &str) -> Option<&'a str> {
    object.get(key).and_then(Value::as_str)
}

fn number_field(object: &serde_json::Map<String, Value>, key: &str) -> Option<u64> {
    object.get(key).and_then(Value::as_u64)
}

fn format_chars(chars: usize) -> String {
    if chars >= 1_000_000 {
        format!("{:.1}m chars", chars as f64 / 1_000_000.0)
    } else if chars >= 1_000 {
        format!("{:.1}k chars", chars as f64 / 1_000.0)
    } else {
        format!("{chars} chars")
    }
}

fn plural_s(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

fn wrapped_line_count(lines: &[Line<'static>], width: usize) -> usize {
    let width = width.max(1);
    lines
        .iter()
        .map(|line| {
            let chars = line
                .spans
                .iter()
                .map(|span| span.content.chars().count())
                .sum::<usize>();
            chars.max(1).div_ceil(width)
        })
        .sum()
}

fn indent_line(line: Line<'static>) -> Line<'static> {
    let mut spans = Vec::with_capacity(line.spans.len() + 1);
    spans.push(Span::raw("  "));
    spans.extend(line.spans);
    Line::from(spans)
}

fn should_handle_key_event(key: KeyEvent) -> bool {
    match key.kind {
        KeyEventKind::Press => true,
        KeyEventKind::Repeat => !matches!(key.code, KeyCode::Char(_)),
        KeyEventKind::Release => false,
    }
}

fn is_toggle_activity_details_key(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('t') | KeyCode::Char('T'))
        && key.modifiers.contains(KeyModifiers::CONTROL)
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use ratatui::text::Line;

    use crate::agent::AgentDisplayEvent;

    use super::{
        ActivityPhase, ChatTui, MessageRole, compact_inline, format_tool_args,
        should_handle_key_event, wrapped_line_count,
    };

    #[test]
    fn ignores_release_and_character_repeat_events() {
        let press = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
        let repeat_char =
            KeyEvent::new_with_kind(KeyCode::Char('s'), KeyModifiers::NONE, KeyEventKind::Repeat);
        let release = KeyEvent::new_with_kind(
            KeyCode::Char('s'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        );
        let repeat_backspace =
            KeyEvent::new_with_kind(KeyCode::Backspace, KeyModifiers::NONE, KeyEventKind::Repeat);

        assert!(should_handle_key_event(press));
        assert!(!should_handle_key_event(repeat_char));
        assert!(!should_handle_key_event(release));
        assert!(should_handle_key_event(repeat_backspace));
    }

    #[test]
    fn assistant_deltas_append_to_current_assistant_message() {
        let mut tui = ChatTui::new(None, std::path::PathBuf::from("."));

        tui.push_assistant_delta("hel");
        tui.push_assistant_delta("lo");

        assert_eq!(tui.messages.len(), 1);
        assert!(matches!(tui.messages[0].role, MessageRole::Assistant));
        assert_eq!(tui.messages[0].body, "hello");
    }

    #[test]
    fn agent_tool_events_group_into_one_transcript_block() {
        let mut tui = ChatTui::new(None, std::path::PathBuf::from("."));

        tui.apply_agent_events(vec![
            AgentDisplayEvent::ToolBatchStart { count: 2 },
            AgentDisplayEvent::ToolCall {
                name: "fs.read".to_string(),
                args: r#"{"path":"README.md"}"#.to_string(),
            },
            AgentDisplayEvent::ToolResult {
                name: "fs.read".to_string(),
                ok: true,
                duration_ms: 12,
                output_chars: 512,
                error: None,
            },
            AgentDisplayEvent::ToolCall {
                name: "fs.search".to_string(),
                args: r#"{"query":"compaction","path":"src"}"#.to_string(),
            },
            AgentDisplayEvent::ToolResult {
                name: "fs.search".to_string(),
                ok: false,
                duration_ms: 3,
                output_chars: 86,
                error: Some("missing path".to_string()),
            },
        ]);

        assert_eq!(tui.messages.len(), 1);
        assert!(matches!(tui.messages[0].role, MessageRole::Tool));
        assert!(tui.messages[0].body.contains("batch 1: 2 calls"));
        assert!(tui.messages[0].body.contains("[1] fs.read README.md"));
        assert!(tui.messages[0].body.contains("ok fs.read 12ms 512 chars"));
        assert!(
            tui.messages[0]
                .body
                .contains("failed fs.search 3ms 86 chars: missing path")
        );
        assert!(matches!(tui.activity.phase, ActivityPhase::Tools));
        assert_eq!(tui.activity.finished_tools, 2);
    }

    #[test]
    fn consecutive_tool_batches_share_one_transcript_block() {
        let mut tui = ChatTui::new(None, std::path::PathBuf::from("."));

        tui.apply_agent_events(vec![
            AgentDisplayEvent::ToolBatchStart { count: 1 },
            AgentDisplayEvent::ToolCall {
                name: "fs.read".to_string(),
                args: r#"{"path":"src/lib.rs","offset":1,"limit":50}"#.to_string(),
            },
            AgentDisplayEvent::ToolResult {
                name: "fs.read".to_string(),
                ok: true,
                duration_ms: 1,
                output_chars: 1_250,
                error: None,
            },
            AgentDisplayEvent::ToolBatchStart { count: 1 },
            AgentDisplayEvent::ToolCall {
                name: "fs.search".to_string(),
                args: r#"{"path":"src","query":"compaction"}"#.to_string(),
            },
            AgentDisplayEvent::ToolResult {
                name: "fs.search".to_string(),
                ok: true,
                duration_ms: 2,
                output_chars: 800,
                error: None,
            },
        ]);

        assert_eq!(tui.messages.len(), 1);
        assert!(matches!(tui.messages[0].role, MessageRole::Tool));
        assert!(tui.messages[0].body.contains("batch 1: 1 call"));
        assert!(tui.messages[0].body.contains("batch 2: 1 call"));
        assert!(tui.messages[0].body.contains("src/lib.rs lines 1..+50"));
        assert!(
            tui.messages[0]
                .body
                .contains(r#"query "compaction" in src"#)
        );
    }

    #[test]
    fn search_tool_args_render_query_field() {
        assert_eq!(
            format_tool_args("fs.search", r#"{"path":"README.md","query":"sessions"}"#),
            r#"query "sessions" in README.md"#
        );
        assert_eq!(
            format_tool_args(
                "fs.search",
                r#"{"path":"src","query":"fn\\s+main","regex":true}"#
            ),
            r#"regex "fn\\s+main" in src"#
        );
    }

    #[test]
    fn tool_transcript_collapses_details_by_default_and_expands_on_toggle() {
        let mut tui = ChatTui::new(None, std::path::PathBuf::from("."));

        tui.apply_agent_events(vec![
            AgentDisplayEvent::ToolBatchStart { count: 1 },
            AgentDisplayEvent::ToolCall {
                name: "fs.read".to_string(),
                args: r#"{"path":"README.md","offset":1,"limit":20}"#.to_string(),
            },
            AgentDisplayEvent::ToolResult {
                name: "fs.read".to_string(),
                ok: true,
                duration_ms: 4,
                output_chars: 1_024,
                error: None,
            },
        ]);

        let collapsed = flatten_lines(&tui.render_transcript_lines());
        assert!(collapsed.contains("1 batch, 1 call, 1 ok"));
        assert!(collapsed.contains("fs.read x1"));
        assert!(collapsed.contains("Ctrl+T expand"));
        assert!(!collapsed.contains("[1] fs.read README.md"));

        tui.toggle_activity_details();
        let expanded = flatten_lines(&tui.render_transcript_lines());
        assert!(expanded.contains("Ctrl+T collapse"));
        assert!(expanded.contains("[1] fs.read README.md lines 1..+20"));
    }

    #[test]
    fn compaction_start_sets_live_activity_and_transcript_notice() {
        let mut tui = ChatTui::new(None, std::path::PathBuf::from("."));

        tui.apply_agent_events(vec![
            AgentDisplayEvent::CompactionStart {
                trigger: Some("size_threshold".to_string()),
                input_chars: 220_000,
            },
            AgentDisplayEvent::CompactionFinish {
                notice: "compaction: responses_compact 220000->80000 chars in 1234ms".to_string(),
            },
        ]);

        assert!(matches!(tui.activity.phase, ActivityPhase::Thinking));
        assert_eq!(tui.messages.len(), 1);
        assert!(matches!(tui.messages[0].role, MessageRole::Compaction));
        assert!(
            tui.messages[0]
                .body
                .contains("running trigger=size_threshold input=220000 chars")
        );
        assert!(
            tui.messages[0]
                .body
                .contains("done compaction: responses_compact 220000->80000 chars in 1234ms")
        );

        let collapsed = flatten_lines(&tui.render_transcript_lines());
        assert!(collapsed.contains("compaction: responses_compact 220000->80000 chars in 1234ms"));
        assert!(collapsed.contains("Ctrl+T expand"));
        assert!(!collapsed.contains("running trigger=size_threshold"));

        tui.toggle_activity_details();
        let expanded = flatten_lines(&tui.render_transcript_lines());
        assert!(expanded.contains("running trigger=size_threshold input=220000 chars"));
    }

    #[test]
    fn compact_inline_truncates_long_status_values() {
        let compact = compact_inline("a ".repeat(120).as_str(), 24);

        assert!(compact.ends_with("..."));
        assert!(compact.chars().count() <= 24);
    }

    #[test]
    fn wrapped_line_count_accounts_for_terminal_width() {
        let lines = vec![ratatui::text::Line::from("abcdefghij")];

        assert_eq!(wrapped_line_count(&lines, 10), 1);
        assert_eq!(wrapped_line_count(&lines, 4), 3);
    }

    fn flatten_lines(lines: &[Line<'static>]) -> String {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
