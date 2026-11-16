use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

fn own_line(line: ratatui::text::Line) -> Line<'static> {
    let spans: Vec<Span<'static>> = line
        .spans
        .into_iter()
        .map(|s| Span::styled(s.content.into_owned(), s.style))
        .collect();
    Line::from(spans)
}

fn prose_lines(text: &str) -> Vec<Line<'static>> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    let rendered = tui_markdown::from_str(text);
    rendered.lines.into_iter().map(own_line).collect()
}

fn fence_lines(body: &str) -> Vec<Line<'static>> {
    let dim = Style::default().add_modifier(Modifier::DIM);
    let width = body
        .lines()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0)
        .max(20);
    let mut lines = vec![Line::styled(format!("┌{}", "─".repeat(width + 2)), dim)];
    for row in body.lines() {
        lines.push(Line::styled(format!("│ {row}"), dim));
    }
    lines.push(Line::styled(format!("└{}", "─".repeat(width + 2)), dim));
    lines
}

pub fn render_markdown_lite(text: &str) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    let mut rest = text;
    loop {
        match rest.find("```") {
            None => {
                out.extend(prose_lines(rest));
                break;
            }
            Some(start) => {
                out.extend(prose_lines(&rest[..start]));
                let after_open = &rest[start + 3..];
                let after_open = after_open.strip_prefix('\n').unwrap_or(after_open);
                match after_open.find("```") {
                    None => {
                        out.extend(fence_lines(after_open));
                        break;
                    }
                    Some(end) => {
                        let body = &after_open[..end];
                        out.extend(fence_lines(body.trim_end_matches('\n')));
                        rest = &after_open[end + 3..];
                    }
                }
            }
        }
    }
    out
}

pub fn dim_style() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

pub fn amber_style() -> Style {
    Style::default().fg(Color::Yellow)
}

pub fn red_style() -> Style {
    Style::default().fg(Color::Red)
}

pub fn operator_style() -> Style {
    Style::default().fg(Color::Cyan)
}

pub fn arx_role_style() -> Style {
    Style::default()
        .fg(Color::Magenta)
        .add_modifier(Modifier::DIM | Modifier::ITALIC)
}

pub fn tool_head_style() -> Style {
    Style::default().fg(Color::Blue)
}

pub fn arrival_style() -> Style {
    Style::default()
        .fg(Color::Magenta)
        .add_modifier(Modifier::BOLD)
}

pub fn plain_line(line: &Line<'static>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}
