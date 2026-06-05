use pulldown_cmark::{CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd};

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const ITALIC: &str = "\x1b[3m";
const STRIKE: &str = "\x1b[9m";

pub(crate) fn print_assistant_message(markdown: &str) {
    print_rendered_markdown(markdown);
}

fn print_rendered_markdown(markdown: &str) {
    let rendered = render_markdown(markdown);
    if rendered.trim().is_empty() {
        return;
    }
    println!("{}", rendered.trim_end());
}

pub(crate) fn render_markdown(markdown: &str) -> String {
    let parser = Parser::new_ext(markdown, Options::all());
    let mut renderer = TerminalMarkdownRenderer::default();

    for event in parser {
        renderer.push_event(event);
    }

    renderer.finish()
}

#[derive(Default)]
struct TerminalMarkdownRenderer {
    out: String,
    at_line_start: bool,
    blank_lines: usize,
    in_code_block: bool,
    code_line_start: bool,
    block_quote_depth: usize,
    list_stack: Vec<ListFrame>,
    link_stack: Vec<CowStr<'static>>,
    image_stack: Vec<CowStr<'static>>,
}

enum ListFrame {
    Unordered,
    Ordered { next: u64 },
}

impl Default for ListFrame {
    fn default() -> Self {
        Self::Unordered
    }
}

impl TerminalMarkdownRenderer {
    fn push_event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(text) => self.push_text(&text),
            Event::Code(code) => {
                self.out.push_str(BOLD);
                self.out.push_str(&code);
                self.out.push_str(RESET);
                self.at_line_start = false;
                self.blank_lines = 0;
            }
            Event::Html(html) | Event::InlineHtml(html) => self.push_text(&html),
            Event::SoftBreak | Event::HardBreak => self.newline(),
            Event::Rule => {
                self.ensure_block_spacing();
                self.push_quote_prefix_if_needed();
                self.out
                    .push_str("----------------------------------------");
                self.newline();
                self.newline();
            }
            Event::FootnoteReference(label) => {
                self.out.push('[');
                self.out.push_str(&label);
                self.out.push(']');
            }
            Event::TaskListMarker(checked) => {
                self.out.push_str(if checked { "[x] " } else { "[ ] " });
                self.at_line_start = false;
                self.blank_lines = 0;
            }
            Event::InlineMath(math) => {
                self.out.push('$');
                self.out.push_str(&math);
                self.out.push('$');
            }
            Event::DisplayMath(math) => {
                self.ensure_block_spacing();
                self.out.push_str(&math);
                self.newline();
                self.newline();
            }
        }
    }

    fn start_tag(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.ensure_line_prefix(),
            Tag::Heading { .. } => {
                self.ensure_block_spacing();
                self.ensure_line_prefix();
                self.out.push_str(BOLD);
            }
            Tag::BlockQuote(_) => {
                self.ensure_block_spacing();
                self.block_quote_depth += 1;
            }
            Tag::CodeBlock(kind) => {
                self.ensure_block_spacing();
                if let CodeBlockKind::Fenced(language) = kind
                    && !language.is_empty()
                {
                    self.out.push_str(DIM);
                    self.out.push_str(language.as_ref());
                    self.out.push_str(RESET);
                    self.newline();
                }
                self.in_code_block = true;
                self.code_line_start = true;
            }
            Tag::HtmlBlock => self.ensure_block_spacing(),
            Tag::List(start) => {
                self.ensure_block_spacing();
                self.list_stack.push(match start {
                    Some(next) => ListFrame::Ordered { next },
                    None => ListFrame::Unordered,
                });
            }
            Tag::Item => self.start_list_item(),
            Tag::DefinitionList => self.ensure_block_spacing(),
            Tag::DefinitionListTitle => {
                self.ensure_line_prefix();
                self.out.push_str(BOLD);
            }
            Tag::DefinitionListDefinition => {
                self.ensure_line_prefix();
                self.out.push_str("  ");
            }
            Tag::Emphasis => self.out.push_str(ITALIC),
            Tag::Strong => self.out.push_str(BOLD),
            Tag::Strikethrough => self.out.push_str(STRIKE),
            Tag::Superscript => self.out.push('^'),
            Tag::Subscript => self.out.push('~'),
            Tag::Link { dest_url, .. } => self.link_stack.push(dest_url.into_static()),
            Tag::Image { dest_url, .. } => {
                self.image_stack.push(dest_url.into_static());
                self.out.push_str("[image: ");
            }
            Tag::Table(_) | Tag::TableHead | Tag::TableRow => self.ensure_line_prefix(),
            Tag::TableCell => {}
            Tag::FootnoteDefinition(label) => {
                self.ensure_block_spacing();
                self.out.push('[');
                self.out.push_str(&label);
                self.out.push_str("] ");
            }
            Tag::MetadataBlock(_) => {}
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.end_block(),
            TagEnd::Heading(_) => {
                self.out.push_str(RESET);
                self.end_block();
            }
            TagEnd::BlockQuote(_) => {
                self.block_quote_depth = self.block_quote_depth.saturating_sub(1);
                self.end_block();
            }
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                self.code_line_start = false;
                self.end_block();
            }
            TagEnd::HtmlBlock => self.end_block(),
            TagEnd::List(_) => {
                self.list_stack.pop();
                self.end_block();
            }
            TagEnd::Item => self.newline(),
            TagEnd::DefinitionList => self.end_block(),
            TagEnd::DefinitionListTitle => {
                self.out.push_str(RESET);
                self.newline();
            }
            TagEnd::DefinitionListDefinition => self.newline(),
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => self.out.push_str(RESET),
            TagEnd::Superscript => self.out.push('^'),
            TagEnd::Subscript => self.out.push('~'),
            TagEnd::Link => {
                if let Some(url) = self.link_stack.pop()
                    && !url.is_empty()
                {
                    self.out.push_str(DIM);
                    self.out.push_str(" (");
                    self.out.push_str(&url);
                    self.out.push(')');
                    self.out.push_str(RESET);
                }
            }
            TagEnd::Image => {
                if let Some(url) = self.image_stack.pop() {
                    self.out.push_str("] ");
                    self.out.push_str(DIM);
                    self.out.push('(');
                    self.out.push_str(&url);
                    self.out.push(')');
                    self.out.push_str(RESET);
                }
            }
            TagEnd::Table | TagEnd::TableHead | TagEnd::TableRow => self.newline(),
            TagEnd::TableCell => self.out.push('\t'),
            TagEnd::FootnoteDefinition => self.end_block(),
            TagEnd::MetadataBlock(_) => {}
        }
    }

    fn push_text(&mut self, text: &str) {
        if self.in_code_block {
            self.push_code_text(text);
            return;
        }

        self.ensure_line_prefix();
        self.out.push_str(text);
        self.at_line_start = false;
        self.blank_lines = 0;
    }

    fn push_code_text(&mut self, text: &str) {
        for segment in text.split_inclusive('\n') {
            if self.code_line_start {
                self.push_quote_prefix_if_needed();
                self.out.push_str("    ");
                self.code_line_start = false;
            }
            self.out.push_str(segment);
            if segment.ends_with('\n') {
                self.at_line_start = true;
                self.code_line_start = true;
                self.blank_lines = 0;
            } else {
                self.at_line_start = false;
                self.blank_lines = 0;
            }
        }
    }

    fn start_list_item(&mut self) {
        self.ensure_line_prefix();
        let marker = match self.list_stack.last_mut() {
            Some(ListFrame::Ordered { next }) => {
                let marker = format!("{next}. ");
                *next += 1;
                marker
            }
            _ => "- ".to_string(),
        };
        self.out.push_str(&marker);
        self.at_line_start = false;
        self.blank_lines = 0;
    }

    fn ensure_block_spacing(&mut self) {
        if self.out.is_empty() {
            self.at_line_start = true;
            return;
        }
        while self.blank_lines < 1 {
            self.newline();
        }
    }

    fn ensure_line_prefix(&mut self) {
        if self.at_line_start {
            self.push_quote_prefix_if_needed();
            self.at_line_start = false;
        }
    }

    fn push_quote_prefix_if_needed(&mut self) {
        for _ in 0..self.block_quote_depth {
            self.out.push_str("| ");
        }
    }

    fn end_block(&mut self) {
        if !self.at_line_start {
            self.newline();
        }
        self.newline();
    }

    fn newline(&mut self) {
        self.out.push('\n');
        self.at_line_start = true;
        self.blank_lines += 1;
    }

    fn finish(self) -> String {
        self.out.trim_end_matches('\n').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{BOLD, RESET, render_markdown};

    #[test]
    fn renders_headings_and_inline_formatting() {
        let rendered = render_markdown("# Title\n\nHello **world** and `code`.");

        assert!(rendered.contains(&format!("{BOLD}Title{RESET}")));
        assert!(rendered.contains(&format!("{BOLD}world{RESET}")));
        assert!(rendered.contains(&format!("{BOLD}code{RESET}")));
        assert!(!rendered.contains("`code`"));
    }

    #[test]
    fn renders_lists_tasks_and_links() {
        let rendered =
            render_markdown("- [x] Done\n- [ ] Todo\n\nSee [docs](https://example.test).");

        assert!(rendered.contains("- [x] Done"));
        assert!(rendered.contains("- [ ] Todo"));
        assert!(rendered.contains("docs"));
        assert!(rendered.contains("(https://example.test)"));
    }

    #[test]
    fn renders_fenced_code_as_indented_code() {
        let rendered = render_markdown("```rust\nfn main() {}\n```");

        assert!(rendered.contains("rust"));
        assert!(rendered.contains("    fn main() {}"));
    }
}
