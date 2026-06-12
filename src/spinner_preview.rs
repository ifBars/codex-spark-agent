use std::io;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};

const TICK_RATE: Duration = Duration::from_millis(90);
const VISIBLE_ROWS: usize = 14;

#[derive(Debug, Clone, Copy)]
pub(crate) struct SpinnerPreviewSet {
    pub(crate) name: &'static str,
    pub(crate) symbols: &'static [&'static str],
}

pub(crate) fn spinner_sets() -> &'static [SpinnerPreviewSet] {
    &SPINNER_SETS
}

pub(crate) fn spinner_frame(set: SpinnerPreviewSet, tick: usize) -> &'static str {
    set.symbols[tick % set.symbols.len()]
}

pub(crate) fn run() -> Result<()> {
    let mut terminal = PreviewTerminal::enter()?;
    let mut app = SpinnerPreview::default();
    let result = app.run(&mut terminal.terminal);
    terminal.exit()?;
    result
}

const SPINNER_SETS: [SpinnerPreviewSet; 23] = [
    SpinnerPreviewSet {
        name: "ASCII",
        symbols: throbber_widgets_tui::ASCII.symbols,
    },
    SpinnerPreviewSet {
        name: "BOX_DRAWING",
        symbols: throbber_widgets_tui::BOX_DRAWING.symbols,
    },
    SpinnerPreviewSet {
        name: "ARROW",
        symbols: throbber_widgets_tui::ARROW.symbols,
    },
    SpinnerPreviewSet {
        name: "DOUBLE_ARROW",
        symbols: throbber_widgets_tui::DOUBLE_ARROW.symbols,
    },
    SpinnerPreviewSet {
        name: "VERTICAL_BLOCK",
        symbols: throbber_widgets_tui::VERTICAL_BLOCK.symbols,
    },
    SpinnerPreviewSet {
        name: "HORIZONTAL_BLOCK",
        symbols: throbber_widgets_tui::HORIZONTAL_BLOCK.symbols,
    },
    SpinnerPreviewSet {
        name: "QUADRANT_BLOCK",
        symbols: throbber_widgets_tui::QUADRANT_BLOCK.symbols,
    },
    SpinnerPreviewSet {
        name: "QUADRANT_BLOCK_CRACK",
        symbols: throbber_widgets_tui::QUADRANT_BLOCK_CRACK.symbols,
    },
    SpinnerPreviewSet {
        name: "WHITE_SQUARE",
        symbols: throbber_widgets_tui::WHITE_SQUARE.symbols,
    },
    SpinnerPreviewSet {
        name: "WHITE_CIRCLE",
        symbols: throbber_widgets_tui::WHITE_CIRCLE.symbols,
    },
    SpinnerPreviewSet {
        name: "BLACK_CIRCLE",
        symbols: throbber_widgets_tui::BLACK_CIRCLE.symbols,
    },
    SpinnerPreviewSet {
        name: "CLOCK",
        symbols: throbber_widgets_tui::CLOCK.symbols,
    },
    SpinnerPreviewSet {
        name: "BRAILLE_ONE",
        symbols: throbber_widgets_tui::BRAILLE_ONE.symbols,
    },
    SpinnerPreviewSet {
        name: "BRAILLE_DOUBLE",
        symbols: throbber_widgets_tui::BRAILLE_DOUBLE.symbols,
    },
    SpinnerPreviewSet {
        name: "BRAILLE_SIX",
        symbols: throbber_widgets_tui::BRAILLE_SIX.symbols,
    },
    SpinnerPreviewSet {
        name: "BRAILLE_SIX_DOUBLE",
        symbols: throbber_widgets_tui::BRAILLE_SIX_DOUBLE.symbols,
    },
    SpinnerPreviewSet {
        name: "BRAILLE_EIGHT",
        symbols: throbber_widgets_tui::BRAILLE_EIGHT.symbols,
    },
    SpinnerPreviewSet {
        name: "BRAILLE_EIGHT_DOUBLE",
        symbols: throbber_widgets_tui::BRAILLE_EIGHT_DOUBLE.symbols,
    },
    SpinnerPreviewSet {
        name: "OGHAM_A",
        symbols: throbber_widgets_tui::OGHAM_A.symbols,
    },
    SpinnerPreviewSet {
        name: "OGHAM_B",
        symbols: throbber_widgets_tui::OGHAM_B.symbols,
    },
    SpinnerPreviewSet {
        name: "OGHAM_C",
        symbols: throbber_widgets_tui::OGHAM_C.symbols,
    },
    SpinnerPreviewSet {
        name: "PARENTHESIS",
        symbols: throbber_widgets_tui::PARENTHESIS.symbols,
    },
    SpinnerPreviewSet {
        name: "CANADIAN",
        symbols: throbber_widgets_tui::CANADIAN.symbols,
    },
];

#[derive(Debug)]
struct SpinnerPreview {
    selected: usize,
    tick: usize,
    last_tick: Instant,
}

impl Default for SpinnerPreview {
    fn default() -> Self {
        Self {
            selected: 0,
            tick: 0,
            last_tick: Instant::now(),
        }
    }
}

impl SpinnerPreview {
    fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        loop {
            terminal.draw(|frame| self.draw(frame.area(), frame.buffer_mut()))?;

            let timeout = TICK_RATE.saturating_sub(self.last_tick.elapsed());
            if event::poll(timeout)? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            break;
                        }
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Down | KeyCode::Char('j') => self.select_next(),
                        KeyCode::Up | KeyCode::Char('k') => self.select_previous(),
                        KeyCode::Home => self.selected = 0,
                        KeyCode::End => self.selected = spinner_sets().len().saturating_sub(1),
                        _ => {}
                    },
                    _ => {}
                }
            }

            if self.last_tick.elapsed() >= TICK_RATE {
                self.tick = self.tick.wrapping_add(1);
                self.last_tick = Instant::now();
            }
        }

        Ok(())
    }

    fn select_next(&mut self) {
        self.selected = (self.selected + 1).min(spinner_sets().len().saturating_sub(1));
    }

    fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn draw(&self, area: Rect, buffer: &mut ratatui::buffer::Buffer) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(6),
                Constraint::Length(4),
            ])
            .split(area);

        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    "spark spinner preview",
                    Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "  q/Esc exit  Up/Down select",
                    Style::new().fg(Color::DarkGray),
                ),
            ]),
            Line::from("Live throbber-widgets-tui sets for choosing Spark activity spinners."),
        ])
        .block(Block::default().borders(Borders::BOTTOM))
        .render(layout[0], buffer);

        Paragraph::new(self.visible_rows())
            .wrap(Wrap { trim: true })
            .render(layout[1], buffer);

        Paragraph::new(self.selected_summary())
            .block(Block::default().borders(Borders::TOP))
            .render(layout[2], buffer);
    }

    fn visible_rows(&self) -> Vec<Line<'static>> {
        let start = self.selected.saturating_sub(VISIBLE_ROWS / 2);
        let end = (start + VISIBLE_ROWS).min(spinner_sets().len());
        spinner_sets()[start..end]
            .iter()
            .enumerate()
            .map(|(offset, set)| {
                let index = start + offset;
                spinner_row(*set, index == self.selected, self.tick)
            })
            .collect()
    }

    fn selected_summary(&self) -> Vec<Line<'static>> {
        let set = spinner_sets()[self.selected];
        vec![
            Line::from(vec![
                Span::styled(
                    spinner_frame(set, self.tick),
                    Style::new()
                        .fg(Color::LightMagenta)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" selected "),
                Span::styled(set.name, Style::new().fg(Color::Cyan)),
                Span::styled(
                    format!(
                        "  frame {}/{}",
                        self.tick % set.symbols.len() + 1,
                        set.symbols.len()
                    ),
                    Style::new().fg(Color::DarkGray),
                ),
            ]),
            Line::from(format!("code: throbber_widgets_tui::{}", set.name)),
        ]
    }
}

fn spinner_row(set: SpinnerPreviewSet, selected: bool, tick: usize) -> Line<'static> {
    let marker = if selected { ">" } else { " " };
    let name_style = if selected {
        Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::Gray)
    };
    let sample = set.symbols.join(" ");
    Line::from(vec![
        Span::styled(marker, Style::new().fg(Color::Cyan)),
        Span::raw(" "),
        Span::styled(format!("{:<22}", set.name), name_style),
        Span::styled(
            spinner_frame(set, tick),
            Style::new()
                .fg(Color::LightMagenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(sample, Style::new().fg(Color::DarkGray)),
    ])
}

struct PreviewTerminal {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl PreviewTerminal {
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

impl Drop for PreviewTerminal {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

#[cfg(test)]
mod tests {
    use super::{spinner_frame, spinner_sets};

    #[test]
    fn spinner_preview_lists_throbber_widget_sets() {
        let names = spinner_sets()
            .iter()
            .map(|set| set.name)
            .collect::<Vec<_>>();

        assert!(names.len() >= 20);
        assert!(names.contains(&"ASCII"));
        assert!(names.contains(&"BRAILLE_EIGHT"));
        assert!(names.contains(&"BRAILLE_SIX_DOUBLE"));
        assert!(names.contains(&"WHITE_CIRCLE"));
        assert!(names.contains(&"HORIZONTAL_BLOCK"));
    }

    #[test]
    fn spinner_preview_frame_wraps_selected_set() {
        let set = spinner_sets()
            .iter()
            .copied()
            .find(|set| set.name == "BRAILLE_EIGHT")
            .expect("braille spinner set");

        assert_eq!(spinner_frame(set, 0), set.symbols[0]);
        assert_eq!(spinner_frame(set, set.symbols.len()), set.symbols[0]);
    }
}
