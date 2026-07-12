//! Small markdown-to-ratatui renderer tuned to the application's restrained
//! preview style.

use crate::ui::theme;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::Modifier,
    text::{Line, Span, Text},
};

pub fn render(src: &str) -> Text<'static> {
    let mut out = Builder::default();
    for event in Parser::new_ext(src, Options::all()) {
        out.event(event);
    }
    out.finish()
}

struct Builder {
    lines: Vec<Line<'static>>,
    spans: Vec<Span<'static>>,
    styles: Vec<ratatui::style::Style>,
    lists: Vec<Option<u64>>,
    heading: Option<(HeadingLevel, usize)>,
    quote_depth: usize,
    code_block: bool,
    at_line_start: bool,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            spans: Vec::new(),
            styles: Vec::new(),
            lists: Vec::new(),
            heading: None,
            quote_depth: 0,
            code_block: false,
            at_line_start: true,
        }
    }
}

impl Builder {
    fn style(&self) -> ratatui::style::Style {
        self.styles
            .iter()
            .copied()
            .fold(ratatui::style::Style::new(), |a, b| a.patch(b))
    }

    fn text(&mut self, value: impl Into<String>) {
        // Code-block (and raw HTML) Text events carry embedded newlines, which
        // a Span cannot represent — split them into separate Lines.
        let value = value.into();
        for (i, part) in value.split('\n').enumerate() {
            if i > 0 {
                self.line();
            }
            if part.is_empty() {
                continue;
            }
            if self.at_line_start {
                if self.quote_depth > 0 {
                    self.spans.push(Span::styled("▎ ", theme::MUTED));
                }
                if self.code_block {
                    self.spans.push(Span::raw("  "));
                }
                self.at_line_start = false;
            }
            if let Some((_, len)) = &mut self.heading {
                *len += part.chars().count();
            }
            self.spans.push(Span::styled(part.to_string(), self.style()));
        }
    }

    fn line(&mut self) {
        self.lines.push(Line::from(std::mem::take(&mut self.spans)));
        self.at_line_start = true;
    }

    fn blank(&mut self) {
        if !self.spans.is_empty() {
            self.line();
        }
        if self.lines.last().is_none_or(|line| !line.spans.is_empty()) {
            self.lines.push(Line::default());
        }
        self.at_line_start = true;
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    self.heading = Some((level, 0));
                    let style = match level {
                        HeadingLevel::H1 => theme::ACCENT.add_modifier(Modifier::BOLD),
                        HeadingLevel::H2 => {
                            ratatui::style::Style::new().add_modifier(Modifier::BOLD)
                        }
                        _ => ratatui::style::Style::new()
                            .add_modifier(Modifier::BOLD | Modifier::DIM),
                    };
                    self.styles.push(style);
                }
                Tag::Emphasis => self
                    .styles
                    .push(ratatui::style::Style::new().add_modifier(Modifier::ITALIC)),
                Tag::Strong => self
                    .styles
                    .push(ratatui::style::Style::new().add_modifier(Modifier::BOLD)),
                Tag::Strikethrough => self
                    .styles
                    .push(ratatui::style::Style::new().add_modifier(Modifier::CROSSED_OUT)),
                Tag::Link { .. } => self
                    .styles
                    .push(theme::ACCENT.add_modifier(Modifier::UNDERLINED)),
                Tag::BlockQuote(_) => {
                    self.quote_depth += 1;
                    self.styles
                        .push(theme::MUTED.add_modifier(Modifier::ITALIC));
                }
                Tag::CodeBlock(_) => {
                    self.blank();
                    self.code_block = true;
                    self.styles.push(theme::MUTED);
                }
                Tag::List(start) => self.lists.push(start),
                Tag::Item => {
                    if !self.spans.is_empty() {
                        self.line();
                    }
                    self.text("  ".repeat(self.lists.len().saturating_sub(1)));
                    let marker = match self.lists.last_mut() {
                        Some(Some(n)) => {
                            let s = format!("{}. ", *n);
                            *n += 1;
                            s
                        }
                        _ => "• ".to_string(),
                    };
                    self.spans.push(Span::styled(marker, theme::MUTED));
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => self.blank(),
                TagEnd::Heading(_) => {
                    self.styles.pop();
                    let (level, len) = self.heading.take().unwrap();
                    self.line();
                    if level == HeadingLevel::H1 {
                        self.lines
                            .push(Line::styled("─".repeat(len.max(1)), theme::RULE));
                    }
                    self.blank();
                }
                TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link => {
                    self.styles.pop();
                }
                TagEnd::BlockQuote(_) => {
                    self.styles.pop();
                    self.quote_depth = self.quote_depth.saturating_sub(1);
                    self.blank();
                }
                TagEnd::CodeBlock => {
                    self.styles.pop();
                    self.code_block = false;
                    self.blank();
                }
                TagEnd::List(_) => {
                    self.lists.pop();
                    self.blank();
                }
                TagEnd::Item => self.line(),
                _ => {}
            },
            Event::Text(s) => self.text(s.into_string()),
            Event::Code(s) => self.spans.push(Span::styled(
                s.into_string(),
                theme::MATCH.remove_modifier(Modifier::BOLD),
            )),
            Event::SoftBreak => self.text(" "),
            Event::HardBreak => self.line(),
            Event::Rule => {
                self.line();
                self.lines.push(Line::styled("─".repeat(24), theme::RULE));
                self.blank();
            }
            Event::Html(s) | Event::InlineHtml(s) | Event::FootnoteReference(s) => {
                self.spans.push(Span::styled(s.into_string(), theme::MUTED));
            }
            Event::TaskListMarker(done) => self.text(if done { "[x] " } else { "[ ] " }),
            _ => {}
        }
    }

    fn finish(mut self) -> Text<'static> {
        if !self.spans.is_empty() {
            self.line();
        }
        while self.lines.last().is_some_and(|line| line.spans.is_empty()) {
            self.lines.pop();
        }
        let leading = self
            .lines
            .iter()
            .take_while(|line| line.spans.is_empty())
            .count();
        self.lines.drain(..leading);
        Text::from(self.lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn h1_has_accent_and_rule() {
        let text = render("# Title\n");
        assert_eq!(text.lines[0].spans[0].style.fg, Some(Color::Blue));
        assert_eq!(text.lines[1].spans[0].content, "─────");
    }

    #[test]
    fn nested_lists_are_indented() {
        let text = render("- one\n  1. two\n");
        assert!(text
            .lines
            .iter()
            .any(|l| l.to_string().starts_with("  1. two")));
    }

    #[test]
    fn inline_code_is_cyan() {
        let text = render("plain `code`");
        assert_eq!(text.lines[0].spans[1].style.fg, Some(Color::Cyan));
    }

    #[test]
    fn code_block_keeps_its_lines() {
        let text = render("```\nfn main() {\n    body();\n}\n```\n");
        let rows: Vec<String> = text.lines.iter().map(|l| l.to_string()).collect();
        assert_eq!(rows, ["  fn main() {", "      body();", "  }"]);
    }

    #[test]
    fn blockquote_has_prefix() {
        assert!(render("> quote").lines[0]
            .to_string()
            .starts_with("▎ quote"));
    }

    #[test]
    fn plain_paragraph_is_unstyled() {
        assert_eq!(
            render("plain").lines[0].spans[0].style,
            ratatui::style::Style::new()
        );
    }
}
