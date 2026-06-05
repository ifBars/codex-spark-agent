use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

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

const DOUBLE_ESCAPE_STOP_WINDOW: Duration = Duration::from_millis(700);
const ACCENT: Color = Color::Cyan;
const MUTED: Color = Color::DarkGray;
const SOFT_TEXT: Color = Color::Gray;
const USER_COLOR: Color = Color::Green;
const TOOL_COLOR: Color = Color::LightBlue;
const REASONING_COLOR: Color = Color::Magenta;
const CONTEXT_COLOR: Color = Color::Yellow;
const WARNING_COLOR: Color = Color::Red;

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
    reasoning_index: Option<usize>,
    activity_details_expanded: bool,
    last_running_escape: Option<Instant>,
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
    Reasoning,
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
            reasoning_index: None,
            activity_details_expanded: false,
            last_running_escape: None,
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
        self.last_running_escape = None;
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
        self.last_running_escape = None;
        self.drain_shared_agent_events(&events);
        self.activity.finish();
        self.tool_group_index = None;
        match run_result {
            Ok(true) => self.push_warning("agent stopped by double Escape"),
            Ok(false) => {}
            Err(error) => self.push_warning(format!("error: {error:#}")),
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
    ) -> Result<bool> {
        let mut run = std::pin::pin!(runner.run(input));
        loop {
            tokio::select! {
                result = &mut run => {
                    self.drain_shared_agent_events(events);
                    terminal.draw(|frame| self.draw(frame))?;
                    return result.map(|_| false);
                }
                _ = tokio::time::sleep(Duration::from_millis(50)) => {
                    self.drain_shared_agent_events(events);
                    if self.handle_running_input()? {
                        self.drain_shared_agent_events(events);
                        terminal.draw(|frame| self.draw(frame))?;
                        return Ok(true);
                    }
                    self.tick_activity();
                    terminal.draw(|frame| self.draw(frame))?;
                }
            }
        }
    }

    fn handle_running_input(&mut self) -> Result<bool> {
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
                KeyCode::Esc if key.kind == KeyEventKind::Press => {
                    if register_escape_tap(&mut self.last_running_escape, Instant::now()) {
                        return Ok(true);
                    }
                    self.push_warning("press Escape again to stop the running agent");
                }
                _ if is_toggle_activity_details_key(key) => self.toggle_activity_details(),
                KeyCode::PageUp => self.scroll_by(-8),
                KeyCode::PageDown => self.scroll_by(8),
                KeyCode::Up => self.scroll_by(-1),
                KeyCode::Down => self.scroll_by(1),
                _ => {}
            }
        }
        Ok(false)
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
Navigation: PageUp/PageDown or Up/Down scrolls, Ctrl+T toggles tool details, Esc clears the composer, double Esc stops a running agent, Ctrl+C exits.",
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
            _ if input.starts_with('/') => {
                self.push_warning(chat::unknown_slash_command_warning(input));
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
                    self.finish_reasoning_message();
                    self.push_assistant(text);
                }
                AgentDisplayEvent::AssistantDelta(text) => {
                    self.activity.receive_response();
                    self.tool_group_index = None;
                    self.finish_reasoning_message();
                    self.push_assistant_delta(&text);
                }
                AgentDisplayEvent::ReasoningStart => {
                    self.activity.start_reasoning();
                    self.start_reasoning_message();
                }
                AgentDisplayEvent::ReasoningSummary(text) => {
                    self.activity.start_reasoning();
                    self.append_reasoning_summary(&text);
                }
                AgentDisplayEvent::ReasoningFinish => {
                    self.activity.finish_reasoning();
                    self.finish_reasoning_message();
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
                    self.finish_reasoning_message();
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
                    self.finish_reasoning_message();
                    self.push_system(text);
                }
                AgentDisplayEvent::Warning(text) => {
                    self.tool_group_index = None;
                    self.finish_reasoning_message();
                    self.push_warning(text);
                }
                AgentDisplayEvent::Profile(text) => {
                    self.tool_group_index = None;
                    self.finish_reasoning_message();
                    self.push_profile(text);
                }
            }
        }
    }

    fn draw(&self, frame: &mut ratatui::Frame<'_>) {
        let command_menu_lines = self.command_menu_lines();
        let command_menu_visible = command_menu_lines.is_some();
        let activity_line = self.activity_line(command_menu_visible);
        let activity_visible = activity_line.is_some();
        let command_menu_height = command_menu_lines
            .as_ref()
            .map(|lines| (lines.len() as u16 + 2).min(8))
            .unwrap_or(0);
        let constraints =
            layout_constraints(command_menu_visible, activity_visible, command_menu_height);
        let chunks = Layout::vertical(constraints).split(frame.area());
        let mut chunk_index = 0;
        let header = chunks[chunk_index];
        chunk_index += 1;
        let activity = if activity_visible {
            let area = chunks[chunk_index];
            chunk_index += 1;
            Some(area)
        } else {
            None
        };
        let transcript = chunks[chunk_index];
        chunk_index += 1;
        let command_menu = if command_menu_visible {
            let area = chunks[chunk_index];
            chunk_index += 1;
            Some(area)
        } else {
            None
        };
        let composer = chunks[chunk_index];
        let footer = chunks[chunk_index + 1];

        frame.render_widget(self.header_line(), header);
        if let (Some(area), Some(line)) = (activity, activity_line) {
            frame.render_widget(line, area);
        }

        let transcript_lines = self.render_transcript_lines();
        let transcript_height = transcript.height.saturating_sub(1) as usize;
        let transcript_width = transcript.width.saturating_sub(1) as usize;
        let tail_scroll = wrapped_line_count(&transcript_lines, transcript_width)
            .saturating_sub(transcript_height) as u16;
        let scroll = tail_scroll.saturating_sub(self.scroll_back);
        let transcript_text = Text::from(transcript_lines);
        let transcript_widget = Paragraph::new(transcript_text)
            .block(
                Block::new()
                    .borders(Borders::LEFT)
                    .border_style(Style::new().fg(Color::Rgb(34, 42, 48))),
            )
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0));
        frame.render_widget(transcript_widget, transcript);

        if let (Some(area), Some(lines)) = (command_menu, command_menu_lines) {
            let menu_widget = Paragraph::new(Text::from(lines))
                .block(
                    Block::new()
                        .borders(Borders::LEFT)
                        .border_style(Style::new().fg(ACCENT))
                        .title(Span::styled(
                            " command palette ",
                            Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
                        )),
                )
                .wrap(Wrap { trim: false });
            frame.render_widget(menu_widget, area);
        }

        let composer_text = if self.input.is_empty() {
            Text::from(Line::from(Span::styled(
                "Ask Spark, or type / for commands",
                Style::new().fg(MUTED),
            )))
        } else {
            Text::from(self.input.as_str())
        };
        let composer_widget = Paragraph::new(composer_text)
            .block(
                Block::new()
                    .borders(Borders::LEFT)
                    .border_style(Style::new().fg(if self.running { MUTED } else { ACCENT }))
                    .title(Span::styled(
                        " message ",
                        Style::new().fg(MUTED).add_modifier(Modifier::BOLD),
                    )),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(composer_widget, composer);

        frame.render_widget(self.footer_line(command_menu_visible), footer);
    }

    fn header_line(&self) -> Line<'static> {
        let session = self
            .session_name
            .as_deref()
            .unwrap_or("workspace")
            .to_string();
        let cwd = self
            .cwd
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(".")
            .to_string();
        let state = if self.running { "running" } else { "ready" };
        let state_color = if self.running { ACCENT } else { USER_COLOR };

        Line::from(vec![
            Span::styled("▌", Style::new().fg(ACCENT).add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(
                "Spark",
                Style::new().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
            muted_span(" "),
            status_span(state, state_color),
            muted_span("  /  "),
            Span::styled(self.activity.phase_label(), self.activity.header_style()),
            muted_span("  ·  session "),
            Span::styled(session, Style::new().fg(SOFT_TEXT)),
            muted_span("  ·  cwd "),
            Span::styled(cwd, Style::new().fg(SOFT_TEXT)),
        ])
    }

    fn activity_line(&self, command_menu_visible: bool) -> Option<Line<'static>> {
        if command_menu_visible {
            return Some(Line::from(vec![
                muted_span("  mode "),
                Span::styled("command palette", Style::new().fg(ACCENT)),
                muted_span(" · "),
                Span::styled(
                    "type to filter, Enter to run, Esc to clear",
                    Style::new().fg(SOFT_TEXT),
                ),
            ]));
        }

        if matches!(self.activity.phase, ActivityPhase::Idle) {
            return None;
        }

        Some(Line::from(vec![
            muted_span("  activity "),
            Span::styled(
                self.activity.live_label(),
                self.activity.header_style().add_modifier(Modifier::BOLD),
            ),
            muted_span(" · "),
            Span::styled(self.activity.detail.clone(), Style::new().fg(SOFT_TEXT)),
            self.activity
                .turn
                .map(|turn| muted_owned(format!(" · turn {turn}")))
                .unwrap_or_else(|| muted_span("")),
            self.activity
                .input_chars
                .map(|chars| muted_owned(format!(" · {}", format_chars(chars))))
                .unwrap_or_else(|| muted_span("")),
        ]))
    }

    fn footer_line(&self, command_menu_visible: bool) -> Line<'static> {
        if self.running {
            return Line::from(vec![
                key_span("Esc Esc"),
                Span::raw(" interrupt"),
                muted_span("  ·  "),
                key_span("Ctrl+T"),
                Span::raw(if self.activity_details_expanded {
                    " collapse details"
                } else {
                    " expand details"
                }),
                muted_span("  ·  "),
                key_span("PgUp/PgDn"),
                Span::raw(" transcript"),
                muted_span("  ·  "),
                key_span("Ctrl+C"),
                Span::raw(" exit"),
            ]);
        }

        if command_menu_visible {
            return Line::from(vec![
                key_span("Enter"),
                Span::raw(" run command"),
                muted_span("  ·  "),
                key_span("Esc"),
                Span::raw(" clear palette"),
                muted_span("  ·  "),
                key_span("PgUp/PgDn"),
                Span::raw(" transcript"),
            ]);
        }

        Line::from(vec![
            key_span("Enter"),
            Span::raw(" send"),
            muted_span("  ·  "),
            key_span("/"),
            Span::raw(" commands"),
            muted_span("  ·  "),
            key_span("Ctrl+T"),
            Span::raw(if self.activity_details_expanded {
                " collapse details"
            } else {
                " expand details"
            }),
            muted_span("  ·  "),
            key_span("PgUp/PgDn"),
            Span::raw(" transcript"),
            muted_span("  ·  "),
            key_span("Ctrl+C"),
            Span::raw(" exit"),
        ])
    }

    fn command_menu_lines(&self) -> Option<Vec<Line<'static>>> {
        if self.running || !self.input.trim_start().starts_with('/') {
            return None;
        }
        let matches = chat::matching_slash_commands(&self.input);
        if matches.is_empty() {
            return Some(vec![Line::from(vec![
                Span::styled(
                    "! ",
                    Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    chat::slash_command_token(&self.input)
                        .unwrap_or(self.input.trim())
                        .to_string(),
                    Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" is not a Spark command", Style::new().fg(SOFT_TEXT)),
                Span::styled("  Enter shows warning", Style::new().fg(MUTED)),
            ])]);
        }

        const MAX_COMMAND_MENU_ROWS: usize = 6;
        let mut lines = matches
            .iter()
            .take(MAX_COMMAND_MENU_ROWS)
            .map(|command| {
                Line::from(vec![
                    Span::styled("▸ ", Style::new().fg(MUTED)),
                    Span::styled(
                        format!("{:<24}", command.usage),
                        Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(command.description.to_string(), Style::new().fg(SOFT_TEXT)),
                ])
            })
            .collect::<Vec<_>>();
        if matches.len() > MAX_COMMAND_MENU_ROWS {
            lines.push(Line::from(Span::styled(
                format!(
                    "{} more commands. Type more to narrow.",
                    matches.len() - MAX_COMMAND_MENU_ROWS
                ),
                Style::new().fg(MUTED),
            )));
        }
        Some(lines)
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
                MessageRole::Reasoning => render_reasoning_lines(
                    &message.body,
                    self.reasoning_index == Some(index)
                        && matches!(self.activity.phase, ActivityPhase::Thinking),
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

    fn start_reasoning_message(&mut self) {
        if self
            .reasoning_index
            .and_then(|index| self.messages.get(index))
            .is_some_and(|message| matches!(message.role, MessageRole::Reasoning))
        {
            return;
        }
        self.push_message(MessageRole::Reasoning, "");
        self.reasoning_index = self.messages.len().checked_sub(1);
    }

    fn append_reasoning_summary(&mut self, summary: &str) {
        let summary = summary.trim();
        if summary.is_empty() {
            return;
        }
        self.start_reasoning_message();
        let Some(index) = self.reasoning_index else {
            return;
        };
        if let Some(message) = self.messages.get_mut(index) {
            if !message.body.trim().is_empty() {
                message.body.push('\n');
            }
            message.body.push_str(summary);
            self.scroll_back = 0;
        }
    }

    fn finish_reasoning_message(&mut self) {
        let Some(index) = self.reasoning_index.take() else {
            return;
        };
        let should_remove = self.messages.get(index).is_some_and(|message| {
            matches!(message.role, MessageRole::Reasoning) && message.body.trim().is_empty()
        });
        if should_remove {
            self.remove_message(index);
        }
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

    fn remove_message(&mut self, index: usize) {
        if index >= self.messages.len() {
            return;
        }
        self.messages.remove(index);
        self.tool_group_index = adjust_removed_index(self.tool_group_index, index);
        self.compaction_index = adjust_removed_index(self.compaction_index, index);
        self.reasoning_index = adjust_removed_index(self.reasoning_index, index);
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

    fn start_reasoning(&mut self) {
        self.phase = ActivityPhase::Thinking;
        self.detail = "reasoning".to_string();
        self.current_tool = None;
    }

    fn finish_reasoning(&mut self) {
        if matches!(self.phase, ActivityPhase::Thinking) {
            self.detail = "waiting for output".to_string();
        }
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

    fn phase_label(&self) -> &'static str {
        match self.phase {
            ActivityPhase::Idle => "idle",
            ActivityPhase::Thinking => "thinking",
            ActivityPhase::Receiving => "receiving",
            ActivityPhase::Compacting => "compacting",
            ActivityPhase::Tools => "tools",
        }
    }

    fn live_label(&self) -> String {
        match self.phase {
            ActivityPhase::Idle => "idle".to_string(),
            ActivityPhase::Thinking => format!("{} thinking", self.spinner()),
            ActivityPhase::Receiving => format!("{} receiving", self.spinner()),
            ActivityPhase::Compacting => format!("{} compacting", self.spinner()),
            ActivityPhase::Tools => {
                if self.total_tools == 0 {
                    format!("{} tools", self.spinner())
                } else {
                    format!(
                        "{} tools {}/{}",
                        self.spinner(),
                        self.finished_tools.min(self.total_tools),
                        self.total_tools
                    )
                }
            }
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

fn layout_constraints(
    command_menu_visible: bool,
    activity_visible: bool,
    command_menu_height: u16,
) -> Vec<Constraint> {
    let mut constraints = vec![Constraint::Length(1)];
    if activity_visible {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Min(5));
    if command_menu_visible {
        constraints.push(Constraint::Length(command_menu_height));
    }
    constraints.push(Constraint::Length(3));
    constraints.push(Constraint::Length(1));
    constraints
}

fn key_span(text: &'static str) -> Span<'static> {
    Span::styled(text, Style::new().fg(ACCENT).add_modifier(Modifier::BOLD))
}

fn status_span(text: &'static str, color: Color) -> Span<'static> {
    Span::styled(text, Style::new().fg(color).add_modifier(Modifier::BOLD))
}

fn muted_span(text: &'static str) -> Span<'static> {
    Span::styled(text, Style::new().fg(MUTED))
}

fn muted_owned(text: String) -> Span<'static> {
    Span::styled(text, Style::new().fg(MUTED))
}

fn role_line(role: MessageRole) -> Line<'static> {
    let (label, color, marker) = match role {
        MessageRole::User => ("you", USER_COLOR, "●"),
        MessageRole::Assistant => ("spark", ACCENT, "◆"),
        MessageRole::System => ("system", CONTEXT_COLOR, "·"),
        MessageRole::Tool => ("tools", TOOL_COLOR, "▣"),
        MessageRole::Reasoning => ("thinking", REASONING_COLOR, "◇"),
        MessageRole::Compaction => ("context", CONTEXT_COLOR, "◈"),
        MessageRole::Warning => ("warning", WARNING_COLOR, "!"),
        MessageRole::Profile => ("profile", TOOL_COLOR, "■"),
    };
    Line::from(vec![
        Span::styled(
            format!("{marker} "),
            Style::new().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            label.to_string(),
            Style::new().fg(color).add_modifier(Modifier::BOLD),
        ),
    ])
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
            return Line::from(vec![
                Span::styled("╭ ", Style::new().fg(MUTED)),
                Span::styled(
                    line.to_string(),
                    Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
                ),
            ]);
        }
        if let Some(rest) = line.strip_prefix("  [") {
            let Some((index, after_index)) = rest.split_once("] ") else {
                return Line::from(Span::raw(line.to_string()));
            };
            let Some((tool, summary)) = after_index.split_once(' ') else {
                return Line::from(Span::raw(line.to_string()));
            };
            return Line::from(vec![
                muted_span("  "),
                Span::styled(format!("[{index}] "), Style::new().fg(MUTED)),
                Span::styled(
                    tool.to_string(),
                    Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(summary.to_string(), Style::new().fg(SOFT_TEXT)),
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
                    muted_span("      "),
                    Span::styled(
                        status.to_string(),
                        Style::new().fg(color).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(suffix.to_string(), Style::new().fg(MUTED)),
                ]);
            }
        }
        Line::from(Span::raw(line.to_string()))
    }));
    lines
}

fn render_reasoning_lines(text: &str, active: bool, spinner: &'static str) -> Vec<Line<'static>> {
    let body = text.trim();
    let state = if active { spinner } else { "done" };
    let state_color = if active { ACCENT } else { Color::Green };
    let summary = if body.is_empty() {
        if active {
            "thinking through the next step"
        } else {
            "reasoning complete"
        }
    } else {
        "reasoning summary"
    };
    let mut lines = vec![Line::from(vec![
        Span::styled("◌ ", Style::new().fg(Color::Magenta)),
        Span::styled(
            state.to_string(),
            Style::new().fg(state_color).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(summary.to_string(), Style::new().fg(SOFT_TEXT)),
    ])];

    if !body.is_empty() {
        lines.extend(body.lines().map(|line| {
            Line::from(vec![
                Span::styled("summary ", Style::new().fg(MUTED)),
                Span::styled(line.to_string(), Style::new().fg(SOFT_TEXT)),
            ])
        }));
    }
    lines
}

fn render_compaction_lines(
    text: &str,
    expanded: bool,
    active: bool,
    spinner: &'static str,
) -> Vec<Line<'static>> {
    let summary = summarize_compaction_text(text);
    let marker = if expanded { "▾" } else { "▸" };
    let state = if active {
        spinner
    } else if summary.done {
        "ok"
    } else {
        ".."
    };
    let mut lines = vec![Line::from(vec![
        Span::styled(format!("{marker} "), Style::new().fg(MUTED)),
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
        Span::styled(summary.text, Style::new().fg(SOFT_TEXT)),
        Span::styled(
            if expanded {
                "  Ctrl+T collapse"
            } else {
                "  Ctrl+T expand"
            },
            Style::new().fg(MUTED),
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
        Line::from(vec![
            Span::styled("  ", Style::new().fg(MUTED)),
            Span::styled(
                line.to_string(),
                Style::new().fg(color).add_modifier(Modifier::BOLD),
            ),
        ])
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
    let marker = if expanded { "▾" } else { "▸" };
    let state = if active {
        format!("{spinner} running")
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
        Span::styled(format!("{marker} "), Style::new().fg(MUTED)),
        Span::styled(
            state,
            Style::new().fg(status_color).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(details, Style::new().fg(SOFT_TEXT)),
        Span::styled(
            if expanded {
                "  Ctrl+T collapse"
            } else {
                "  Ctrl+T expand"
            },
            Style::new().fg(MUTED),
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
    spans.push(Span::styled("│ ", Style::new().fg(MUTED)));
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

fn adjust_removed_index(value: Option<usize>, removed: usize) -> Option<usize> {
    match value {
        Some(index) if index == removed => None,
        Some(index) if index > removed => Some(index - 1),
        other => other,
    }
}

fn register_escape_tap(last_escape: &mut Option<Instant>, now: Instant) -> bool {
    let is_double_tap = last_escape
        .map(|last| now.duration_since(last) <= DOUBLE_ESCAPE_STOP_WINDOW)
        .unwrap_or(false);
    *last_escape = Some(now);
    is_double_tap
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::text::Line;

    use crate::agent::AgentDisplayEvent;

    use super::{
        ActivityPhase, ChatTui, MessageRole, compact_inline, format_tool_args, register_escape_tap,
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
    fn escape_double_tap_requires_second_press_inside_window() {
        let start = Instant::now();
        let mut last_escape = None;

        assert!(!register_escape_tap(&mut last_escape, start));
        assert!(register_escape_tap(
            &mut last_escape,
            start + Duration::from_millis(250)
        ));

        let mut last_escape = None;
        assert!(!register_escape_tap(&mut last_escape, start));
        assert!(!register_escape_tap(
            &mut last_escape,
            start + Duration::from_secs(2)
        ));
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
    fn reasoning_row_is_live_and_transient_without_summary() {
        let mut tui = ChatTui::new(None, std::path::PathBuf::from("."));

        tui.apply_agent_events(vec![AgentDisplayEvent::ReasoningStart]);
        let rendered = flatten_lines(&tui.render_transcript_lines());
        assert!(rendered.contains("thinking"));
        assert!(rendered.contains("thinking through the next step"));
        assert!(matches!(tui.activity.phase, ActivityPhase::Thinking));

        tui.apply_agent_events(vec![AgentDisplayEvent::ReasoningFinish]);
        assert!(tui.messages.is_empty());
    }

    #[test]
    fn reasoning_summary_row_is_kept_after_finish() {
        let mut tui = ChatTui::new(None, std::path::PathBuf::from("."));

        tui.apply_agent_events(vec![
            AgentDisplayEvent::ReasoningStart,
            AgentDisplayEvent::ReasoningSummary("Checked the edited files.".to_string()),
            AgentDisplayEvent::ReasoningFinish,
        ]);

        assert_eq!(tui.messages.len(), 1);
        assert!(matches!(tui.messages[0].role, MessageRole::Reasoning));
        let rendered = flatten_lines(&tui.render_transcript_lines());
        assert!(rendered.contains("reasoning summary"));
        assert!(rendered.contains("Checked the edited files."));
    }

    #[test]
    fn slash_command_menu_filters_commands_and_marks_unknown() {
        let mut tui = ChatTui::new(None, std::path::PathBuf::from("."));
        tui.input = "/sk".to_string();

        let rendered = flatten_lines(&tui.command_menu_lines().expect("menu"));
        assert!(rendered.contains("/skill"));
        assert!(rendered.contains("/skills"));

        tui.input = "/wat".to_string();
        let rendered = flatten_lines(&tui.command_menu_lines().expect("unknown menu"));
        assert!(rendered.contains("is not a Spark command"));
        assert!(rendered.contains("/wat"));
    }

    #[test]
    fn activity_rail_reflects_command_palette_and_running_state() {
        let mut tui = ChatTui::new(
            Some("design-pass".to_string()),
            std::path::PathBuf::from("C:/workspace/spark"),
        );

        let ready = flatten_lines(&[tui.header_line()]);
        assert!(ready.contains("Spark"));
        assert!(ready.contains("ready"));
        assert!(ready.contains("session design-pass"));
        assert!(ready.contains("cwd spark"));
        assert!(tui.activity_line(false).is_none());

        tui.input = "/sta".to_string();
        let palette = flatten_lines(&[
            tui.activity_line(true).expect("palette line"),
            tui.footer_line(true),
        ]);
        assert!(palette.contains("command palette"));
        assert!(palette.contains("Enter run command"));

        tui.running = true;
        tui.activity.start_tools(3);
        tui.activity.start_tool("fs.read");
        let running = flatten_lines(&[
            tui.activity_line(false).expect("running line"),
            tui.footer_line(false),
        ]);
        assert!(running.contains("tools 0/3"));
        assert!(running.contains("running fs.read"));
        assert!(running.contains("Esc Esc interrupt"));
    }

    #[test]
    fn draw_renders_agent_console_rails() {
        let mut tui = ChatTui::new(
            Some("visual-test".to_string()),
            std::path::PathBuf::from("C:/workspace/spark"),
        );
        tui.push_system("Type /help for commands.");
        tui.push_user("Polish the TUI");
        tui.apply_agent_events(vec![
            AgentDisplayEvent::ReasoningStart,
            AgentDisplayEvent::ReasoningSummary("Checking Ratatui references.".to_string()),
            AgentDisplayEvent::ReasoningFinish,
            AgentDisplayEvent::ToolBatchStart { count: 1 },
            AgentDisplayEvent::ToolCall {
                name: "fs.read".to_string(),
                args: r#"{"path":"src/chat_tui.rs","offset":1,"limit":20}"#.to_string(),
            },
            AgentDisplayEvent::ToolResult {
                name: "fs.read".to_string(),
                ok: true,
                duration_ms: 5,
                output_chars: 900,
                error: None,
            },
        ]);

        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|frame| tui.draw(frame)).expect("draw");

        let rendered = buffer_to_string(terminal.backend().buffer());
        assert!(rendered.contains("Spark"));
        assert!(rendered.contains("session visual-test"));
        assert!(rendered.contains("thinking"));
        assert!(rendered.contains("1 batch, 1 call, 1 ok"));
        assert!(rendered.contains("Enter send"));
        assert!(rendered.contains("/ commands"));
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

    fn buffer_to_string(buffer: &Buffer) -> String {
        let area = buffer.area();
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
