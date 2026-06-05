use pulldown_cmark::{CodeBlockKind, CowStr, Event as MarkdownEvent, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

pub(super) fn render_markdown_lines(markdown: &str) -> Vec<Line<'static>> {
    let mut renderer = TuiMarkdownRenderer::default();
    for event in Parser::new_ext(markdown, Options::all()) {
        renderer.push_event(event);
    }
    renderer.finish()
}

#[derive(Default)]
struct TuiMarkdownRenderer {
    lines: Vec<Line<'static>>,
    current: Vec<Span<'static>>,
    style_stack: Vec<Style>,
    list_stack: Vec<TuiListFrame>,
    link_stack: Vec<CowStr<'static>>,
    block_quote_depth: usize,
    heading: bool,
    in_code_block: bool,
    code_language: Option<String>,
    code_buffer: String,
}

enum TuiListFrame {
    Unordered,
    Ordered { next: u64 },
}

impl TuiMarkdownRenderer {
    fn push_event(&mut self, event: MarkdownEvent<'_>) {
        if self.in_code_block {
            self.push_code_event(event);
            return;
        }

        match event {
            MarkdownEvent::Start(tag) => self.start_tag(tag),
            MarkdownEvent::End(tag) => self.end_tag(tag),
            MarkdownEvent::Text(text) => self.push_text(&text),
            MarkdownEvent::Code(code) => self.push_styled_text(
                code.to_string(),
                Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            MarkdownEvent::Html(html) | MarkdownEvent::InlineHtml(html) => self.push_text(&html),
            MarkdownEvent::SoftBreak | MarkdownEvent::HardBreak => self.flush_line(),
            MarkdownEvent::Rule => {
                self.ensure_blank_line();
                self.lines.push(Line::from(Span::styled(
                    "----------------------------------------",
                    Style::new().fg(Color::DarkGray),
                )));
                self.ensure_blank_line();
            }
            MarkdownEvent::FootnoteReference(label) => self.push_text(&format!("[{label}]")),
            MarkdownEvent::TaskListMarker(checked) => {
                self.push_text(if checked { "[x] " } else { "[ ] " });
            }
            MarkdownEvent::InlineMath(math) => self.push_text(&format!("${math}$")),
            MarkdownEvent::DisplayMath(math) => {
                self.ensure_blank_line();
                self.push_text(&math);
                self.flush_line();
                self.ensure_blank_line();
            }
        }
    }

    fn push_code_event(&mut self, event: MarkdownEvent<'_>) {
        match event {
            MarkdownEvent::End(TagEnd::CodeBlock) => {
                self.in_code_block = false;
                let language = self.code_language.take().unwrap_or_default();
                let body = std::mem::take(&mut self.code_buffer);
                if is_markdown_language(&language) {
                    self.ensure_blank_line();
                    self.lines.extend(render_markdown_lines(&body));
                    self.ensure_blank_line();
                } else {
                    self.ensure_blank_line();
                    self.push_code_block(&language, &body);
                    self.ensure_blank_line();
                }
            }
            MarkdownEvent::Text(text) => self.code_buffer.push_str(&text),
            MarkdownEvent::SoftBreak | MarkdownEvent::HardBreak => self.code_buffer.push('\n'),
            _ => {}
        }
    }

    fn push_code_block(&mut self, language: &str, body: &str) {
        let language = normalized_language(language);
        let label = if language.is_empty() {
            "code"
        } else {
            language.as_str()
        };
        self.lines.push(Line::from(vec![
            Span::styled("  [ ", Style::new().fg(Color::DarkGray)),
            Span::styled(
                label.to_string(),
                Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ]", Style::new().fg(Color::DarkGray)),
        ]));

        for raw_line in body.lines() {
            let mut spans = vec![Span::styled(
                "  | ",
                Style::new()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )];
            spans.extend(highlight_code_line(&language, raw_line));
            self.lines.push(Line::from(spans));
        }
        if body.ends_with('\n') {
            let spans = vec![Span::styled(
                "  | ",
                Style::new()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )];
            self.lines.push(Line::from(spans));
        }
    }

    fn start_tag(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {}
            Tag::Heading { .. } => {
                self.ensure_blank_line();
                self.heading = true;
                self.style_stack
                    .push(Style::new().fg(Color::White).add_modifier(Modifier::BOLD));
            }
            Tag::BlockQuote(_) => {
                self.ensure_blank_line();
                self.block_quote_depth += 1;
            }
            Tag::CodeBlock(kind) => {
                self.in_code_block = true;
                self.code_language = Some(match kind {
                    CodeBlockKind::Fenced(language) => language.to_string(),
                    CodeBlockKind::Indented => String::new(),
                });
                self.code_buffer.clear();
            }
            Tag::List(start) => {
                self.ensure_blank_line();
                self.list_stack.push(match start {
                    Some(next) => TuiListFrame::Ordered { next },
                    None => TuiListFrame::Unordered,
                });
            }
            Tag::Item => self.start_list_item(),
            Tag::Emphasis => self
                .style_stack
                .push(Style::new().add_modifier(Modifier::ITALIC)),
            Tag::Strong => self
                .style_stack
                .push(Style::new().add_modifier(Modifier::BOLD)),
            Tag::Strikethrough => self
                .style_stack
                .push(Style::new().add_modifier(Modifier::CROSSED_OUT)),
            Tag::Link { dest_url, .. } => {
                self.style_stack.push(Style::new().fg(Color::Cyan));
                self.link_stack.push(dest_url.into_static());
            }
            Tag::Image { dest_url, .. } => {
                self.push_styled_text("[image: ", Style::new().fg(Color::DarkGray));
                self.push_link_marker(dest_url.into_static());
            }
            Tag::Table(_)
            | Tag::TableHead
            | Tag::TableRow
            | Tag::TableCell
            | Tag::HtmlBlock
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::FootnoteDefinition(_)
            | Tag::MetadataBlock(_)
            | Tag::Superscript
            | Tag::Subscript => {}
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.end_block(),
            TagEnd::Heading(_) => {
                self.style_stack.pop();
                self.heading = false;
                self.end_block();
            }
            TagEnd::BlockQuote(_) => {
                self.block_quote_depth = self.block_quote_depth.saturating_sub(1);
                self.end_block();
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
                self.ensure_blank_line();
            }
            TagEnd::Item => self.flush_line(),
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.style_stack.pop();
            }
            TagEnd::Link => {
                self.style_stack.pop();
                if let Some(dest_url) = self.link_stack.pop() {
                    self.push_link_marker(dest_url);
                }
            }
            TagEnd::Image => self.push_styled_text("]", Style::new().fg(Color::DarkGray)),
            TagEnd::Table | TagEnd::TableHead | TagEnd::TableRow => self.flush_line(),
            TagEnd::TableCell => self.push_text(" | "),
            TagEnd::CodeBlock
            | TagEnd::HtmlBlock
            | TagEnd::FootnoteDefinition
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::MetadataBlock(_)
            | TagEnd::Superscript
            | TagEnd::Subscript => {}
        }
    }

    fn push_text(&mut self, text: &str) {
        self.push_styled_text(text.to_string(), self.current_style());
    }

    fn push_styled_text(&mut self, text: impl Into<String>, style: Style) {
        let text = text.into();
        for segment in text.split_inclusive('\n') {
            let had_newline = segment.ends_with('\n');
            let segment = segment.strip_suffix('\n').unwrap_or(segment);
            if !segment.is_empty() {
                self.ensure_prefix();
                self.current.push(Span::styled(segment.to_string(), style));
            }
            if had_newline {
                self.flush_line();
            }
        }
    }

    fn push_link_marker(&mut self, dest_url: CowStr<'static>) {
        if !dest_url.is_empty() {
            self.current.push(Span::styled(
                format!(" ({dest_url}) "),
                Style::new().fg(Color::DarkGray),
            ));
        }
    }

    fn start_list_item(&mut self) {
        self.ensure_prefix();
        let marker = match self.list_stack.last_mut() {
            Some(TuiListFrame::Ordered { next }) => {
                let marker = format!("{next}. ");
                *next += 1;
                marker
            }
            _ => "- ".to_string(),
        };
        self.current.push(Span::styled(
            marker,
            Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ));
    }

    fn ensure_prefix(&mut self) {
        if self.current.is_empty() {
            for _ in 0..self.block_quote_depth {
                self.current.push(Span::styled(
                    "| ",
                    Style::new()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                ));
            }
        }
    }

    fn current_style(&self) -> Style {
        let mut style = if self.heading {
            Style::new().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::new()
        };
        for stacked in &self.style_stack {
            style = style.patch(*stacked);
        }
        style
    }

    fn end_block(&mut self) {
        self.flush_line();
        self.ensure_blank_line();
    }

    fn flush_line(&mut self) {
        if self.current.is_empty() {
            return;
        }
        self.lines
            .push(Line::from(std::mem::take(&mut self.current)));
    }

    fn ensure_blank_line(&mut self) {
        self.flush_line();
        if self.lines.last().is_some_and(|line| !line.spans.is_empty()) {
            self.lines.push(Line::from(""));
        }
    }

    fn finish(mut self) -> Vec<Line<'static>> {
        self.flush_line();
        while self.lines.last().is_some_and(|line| line.spans.is_empty()) {
            self.lines.pop();
        }
        if self.lines.is_empty() {
            self.lines.push(Line::from(""));
        }
        self.lines
    }
}

fn normalized_language(language: &str) -> String {
    let language = language
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    match language.as_str() {
        "cs" | "c#" => "csharp".to_string(),
        "rs" => "rust".to_string(),
        "js" => "javascript".to_string(),
        "ts" => "typescript".to_string(),
        other => other.to_string(),
    }
}

fn highlight_code_line(language: &str, line: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut chars = line.char_indices().peekable();
    while let Some((start, ch)) = chars.next() {
        if ch == '/' && matches!(chars.peek(), Some((_, '/'))) {
            let comment = &line[start..];
            spans.push(Span::styled(
                comment.to_string(),
                Style::new()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ));
            break;
        }

        if matches!(ch, '"' | '\'') {
            let quote = ch;
            let mut end = start + ch.len_utf8();
            let mut escaped = false;
            for (index, next) in chars.by_ref() {
                end = index + next.len_utf8();
                if escaped {
                    escaped = false;
                    continue;
                }
                if next == '\\' {
                    escaped = true;
                    continue;
                }
                if next == quote {
                    break;
                }
            }
            spans.push(Span::styled(
                line[start..end].to_string(),
                Style::new().fg(Color::Green),
            ));
            continue;
        }

        if ch.is_ascii_digit() {
            let mut end = start + ch.len_utf8();
            while let Some((index, next)) = chars.peek().copied() {
                if next.is_ascii_alphanumeric() || matches!(next, '.' | '_') {
                    chars.next();
                    end = index + next.len_utf8();
                } else {
                    break;
                }
            }
            spans.push(Span::styled(
                line[start..end].to_string(),
                Style::new().fg(Color::Magenta),
            ));
            continue;
        }

        if is_ident_start(ch) {
            let mut end = start + ch.len_utf8();
            while let Some((index, next)) = chars.peek().copied() {
                if is_ident_continue(next) {
                    chars.next();
                    end = index + next.len_utf8();
                } else {
                    break;
                }
            }
            let ident = &line[start..end];
            spans.push(Span::styled(
                ident.to_string(),
                code_identifier_style(language, ident),
            ));
            continue;
        }

        spans.push(Span::styled(
            ch.to_string(),
            if ch.is_whitespace() {
                Style::new()
            } else {
                Style::new().fg(Color::Gray)
            },
        ));
    }
    spans
}

fn code_identifier_style(language: &str, ident: &str) -> Style {
    if is_keyword(language, ident) {
        return Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    }
    if is_literal(ident) {
        return Style::new().fg(Color::Magenta);
    }
    if ident.chars().next().is_some_and(char::is_uppercase) {
        return Style::new().fg(Color::Yellow);
    }
    Style::new().fg(Color::Gray)
}

fn is_keyword(language: &str, ident: &str) -> bool {
    let common = matches!(
        ident,
        "async"
            | "await"
            | "break"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "else"
            | "enum"
            | "for"
            | "foreach"
            | "fn"
            | "function"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "match"
            | "mut"
            | "new"
            | "private"
            | "protected"
            | "pub"
            | "public"
            | "return"
            | "static"
            | "struct"
            | "trait"
            | "try"
            | "type"
            | "using"
            | "var"
            | "void"
            | "while"
    );
    common
        || match language {
            "csharp" => matches!(
                ident,
                "base"
                    | "bool"
                    | "decimal"
                    | "default"
                    | "double"
                    | "float"
                    | "int"
                    | "internal"
                    | "namespace"
                    | "object"
                    | "override"
                    | "readonly"
                    | "sealed"
                    | "string"
                    | "this"
                    | "virtual"
            ),
            "rust" => matches!(
                ident,
                "crate" | "dyn" | "extern" | "Self" | "self" | "super" | "unsafe" | "where"
            ),
            "javascript" | "typescript" => matches!(
                ident,
                "export" | "extends" | "from" | "import" | "interface" | "of"
            ),
            _ => false,
        }
}

fn is_literal(ident: &str) -> bool {
    matches!(ident, "false" | "null" | "None" | "none" | "nil" | "true")
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_markdown_language(language: &str) -> bool {
    matches!(
        normalized_language(language).as_str(),
        "markdown" | "md" | "mdown" | "mkd"
    )
}

#[cfg(test)]
mod tests {
    use ratatui::style::Modifier;
    use ratatui::text::Line;

    use super::render_markdown_lines;

    #[test]
    fn tui_markdown_renders_styled_spans() {
        let lines = render_markdown_lines("# Title\n\nHello **world** and `code`.");
        let flattened = flatten_lines(&lines);

        assert!(flattened.contains("Title"));
        assert!(flattened.contains("Hello world and code."));
        assert!(!flattened.contains("`code`"));
        assert!(lines.iter().any(|line| {
            line.spans.iter().any(|span| {
                span.content == "world" && span.style.add_modifier.contains(Modifier::BOLD)
            })
        }));
        assert!(lines.iter().any(|line| {
            line.spans.iter().any(|span| {
                span.content == "code" && span.style.add_modifier.contains(Modifier::BOLD)
            })
        }));
    }

    #[test]
    fn markdown_fences_render_as_nested_markdown() {
        let lines = render_markdown_lines("```markdown\n# Project\n\n- Done\n```");
        let flattened = flatten_lines(&lines);

        assert!(flattened.contains("Project"));
        assert!(flattened.contains("- Done"));
        assert!(!flattened.contains("```"));
        assert!(!flattened.contains("markdown"));
    }

    #[test]
    fn fenced_code_blocks_get_headers_and_token_styles() {
        let lines = render_markdown_lines(
            "```csharp\nvar score = client.CreateHostSyncVar<int>(key: \"Score\", defaultValue: 0); // optional\n```",
        );
        let flattened = flatten_lines(&lines);

        assert!(flattened.contains("[ csharp ]"));
        assert!(flattened.contains("| var score = client.CreateHostSyncVar<int>"));
        assert!(lines.iter().any(|line| {
            line.spans.iter().any(|span| {
                span.content == "var" && span.style.add_modifier.contains(Modifier::BOLD)
            })
        }));
        assert!(lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.content == "\"Score\"" && span.style.fg.is_some())
        }));
        assert!(lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.content == "// optional" && span.style.fg.is_some())
        }));
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
