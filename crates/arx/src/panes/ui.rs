use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::panes::app::{is_split_party, App, FocusTarget, Half, ModelPickerTab, Mode, Pane, PaneKey, PanelId, PartyKey, ScrollState};
use crate::panes::keys::{KeyMap, KeyTable, INSERT_ACTIONS, MODAL_ACTIONS, NORMAL_ACTIONS, SPLIT_ACTIONS};
use crate::panes::state::{HandRow, Outcome, Role, SessionState, Status, Turn, SPINNER};
use crate::panes::trace::TraceTurn;
use crate::terminal::RosterModel;

pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();
    let root = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(size);
    let cols = Layout::horizontal([Constraint::Percentage(66), Constraint::Percentage(34)]).split(root[0]);

    let panel_rects = draw_right_column(f, app, cols[1]);
    app.set_layout_rects(cols[0], panel_rects);
    draw_big_view(f, app, cols[0]);
    draw_hint_bar(f, app, root[1]);

    match app.modal.clone() {
        Some(crate::panes::app::Modal::ModelPicker { models, tabs, tab, main_family }) => {
            draw_model_picker(f, size, &models, &tabs, tab, &main_family)
        }
        Some(crate::panes::app::Modal::Confirm { text, input, action }) => draw_confirm_modal(f, size, &text, &input, &action),
        None => {}
    }
    if app.help_visible {
        draw_help_overlay(f, size, &app.keys);
    }
}

fn wrapped_rows(line: &Line, width: usize) -> usize {
    let text_width: usize = line.spans.iter().map(|s| UnicodeWidthStr::width(s.content.as_ref())).sum();
    if text_width == 0 {
        1
    } else {
        text_width.div_ceil(width.max(1))
    }
}

fn content_fingerprint(lines: &[Line]) -> usize {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    for line in lines {
        for span in &line.spans {
            span.content.hash(&mut hasher);
            0u8.hash(&mut hasher);
        }
        1u8.hash(&mut hasher);
    }
    hasher.finish() as usize
}

fn party_title(app: &App) -> String {
    match app.current_party() {
        PartyKey::Session => "session".to_string(),
        PartyKey::Main(i) => app.state.mains.get(i).map(|m| m.label.clone()).unwrap_or_else(|| "main".to_string()),
        PartyKey::Reviewer(i) => {
            app.state.reviewers.get(i).map(|h| format!("{} {}", h.id, h.agent)).unwrap_or_else(|| "reviewer".to_string())
        }
        PartyKey::Hand(i) => {
            app.state.hands.get(i).map(|h| format!("{} {}", h.id, h.agent)).unwrap_or_else(|| "hand".to_string())
        }
    }
}

fn party_short(app: &App) -> String {
    match app.current_party() {
        PartyKey::Hand(i) => app.state.hands.get(i).map(|h| h.id.clone()).unwrap_or_default(),
        PartyKey::Reviewer(i) => app.state.reviewers.get(i).map(|h| h.id.clone()).unwrap_or_default(),
        _ => String::new(),
    }
}

fn brief_pane_lines(row: &HandRow, session_started: std::time::Instant, width: u16) -> Vec<Line<'static>> {
    let role = match row.role {
        Role::Hand => "hand",
        Role::Reviewer => "reviewer",
    };
    let spawn_t = row.spawned_at.saturating_duration_since(session_started).as_secs_f32();
    let mut lines = vec![
        Line::from(Span::styled(
            format!(
                "{} \u{b7} {} \u{b7} {role} \u{b7} {} \u{b7} {} \u{b7} {} \u{b7} t+{spawn_t:.1}s",
                row.id, row.agent, row.body, row.image, row.model
            ),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    lines.extend(render_markdown(&row.brief, width));
    lines
}

fn cut_to_width(text: &str, width: usize) -> String {
    let width = width.max(1);
    if text.chars().count() <= width {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(width.saturating_sub(1)).collect();
        format!("{truncated}\u{2026}")
    }
}

fn render_trace_turns(turns: &[TraceTurn], width: usize) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for turn in turns {
        match turn {
            TraceTurn::Text(text) => out.extend(render_markdown(text, width as u16)),
            TraceTurn::ToolCall { name, args } => {
                let prefix = format!("\u{b7} {name} ");
                let budget = width.saturating_sub(prefix.chars().count());
                out.push(Line::from(Span::styled(
                    format!("{prefix}{}", cut_to_width(args, budget)),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            TraceTurn::ToolResult(text) => {
                for l in text.lines().take(4) {
                    out.push(Line::from(Span::styled(format!("  {l}"), Style::default().fg(Color::DarkGray))));
                }
            }
            TraceTurn::Diff(text) => {
                out.push(Line::from(Span::styled("worktree.diff", Style::default().add_modifier(Modifier::BOLD))));
                for l in text.lines() {
                    out.push(Line::from(l.to_string()));
                }
            }
        }
    }
    out
}

fn turns_pane_lines(row: &HandRow, width: usize) -> Vec<Line<'static>> {
    match row.outcome {
        None => vec![Line::from(Span::styled(
            format!("running \u{b7} {}", row.status_cell()),
            Style::default().fg(Color::DarkGray),
        ))],
        Some(Outcome::Stop) => {
            let mut lines = vec![Line::from(Span::styled(
                format!("stopped \u{b7} {}", row.status_cell()),
                Style::default().fg(Color::Yellow),
            ))];
            lines.push(Line::from(""));
            if let Some(question) = &row.verdict_text {
                lines.extend(render_markdown(question, width as u16));
            }
            lines
        }
        Some(Outcome::Killed) => verdict_block_lines(row, width),
        Some(Outcome::Verdict) => {
            let mut lines = match &row.trace_turns {
                Some(Ok(turns)) => render_trace_turns(turns, width),
                Some(Err(err)) => {
                    vec![Line::from(Span::styled(format!("trace unreadable: {err}"), Style::default().fg(Color::Red)))]
                }
                None => Vec::new(),
            };
            lines.push(Line::from(""));
            lines.extend(verdict_block_lines(row, width));
            lines
        }
    }
}

fn verdict_block_lines(row: &HandRow, width: usize) -> Vec<Line<'static>> {
    let outcome_word = match row.outcome {
        Some(Outcome::Verdict) => "verdict",
        Some(Outcome::Stop) => "stop",
        Some(Outcome::Killed) => "killed",
        None => "-",
    };
    let mut lines = vec![Line::from(Span::styled(
        format!("outcome   {outcome_word}"),
        Style::default().add_modifier(Modifier::BOLD),
    ))];
    if let Some(text) = &row.verdict_text {
        lines.push(Line::from(""));
        lines.extend(render_markdown(text, width as u16));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(format!("trace     {}", row.trace.clone().unwrap_or_default())));
    lines.push(Line::from(format!("bytes     {}", row.bytes.unwrap_or(0))));
    if let Some(diff) = &row.diff {
        lines.push(Line::from(format!("diff      {diff}")));
    }
    lines.push(Line::from(format!(
        "elapsed   {} ms \u{b7} lag {} ms",
        row.elapsed_ms.unwrap_or(0),
        row.lag_ms.unwrap_or(0)
    )));
    if let Some(ok) = row.schema_ok {
        lines.push(Line::from(format!("schema    {}", if ok { "ok" } else { "malformed" })));
    }
    lines
}

fn session_page_lines(state: &SessionState) -> Vec<Line<'static>> {
    let s = &state.status;
    let mut lines = vec![
        Line::from(Span::styled("session", Style::default().add_modifier(Modifier::BOLD))),
        Line::from(format!("model       {}", s.model)),
        Line::from(""),
        occupancy_bar(s, 60),
        Line::from(format!("occupancy   {} / {}", s.occupancy, s.window)),
        Line::from(format!("reserve     {}", s.reserve)),
    ];
    match s.checkpoint {
        Some((n, delta)) => lines.push(Line::from(format!("checkpoint  {n} (+{delta})"))),
        None => lines.push(Line::from("checkpoint  none yet")),
    }
    lines.push(Line::from(format!("turns       {}", s.turn)));
    lines.push(Line::from(format!("hands in flight  {}", state.hands_in_flight())));
    lines.push(Line::from(""));
    lines.push(Line::from(format!("gate allow {} \u{b7} deny {}", state.gate.allow, state.gate.deny)));
    lines.push(Line::from(""));
    lines.push(Line::from("models"));
    let models: Vec<String> = if state.last_models.is_empty() { vec![s.model.clone()] } else { state.last_models.clone() };
    for m in &models {
        let marker = if *m == s.model { "\u{25b8} " } else { "  " };
        let tag = if *m == s.model { " (current)" } else { "" };
        lines.push(Line::from(format!("{marker}{m}{tag}")));
    }
    lines
}

fn draw_scrollable_pane(
    f: &mut Frame,
    app: &mut App,
    area: Rect,
    key: PaneKey,
    lines: Vec<Line<'static>>,
    title: &str,
    border_style: Style,
) {
    let block = Block::default().borders(Borders::ALL).title(title.to_string()).border_style(border_style);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let avail_width = inner.width.max(1) as usize;
    let source_len = content_fingerprint(&lines);
    let total_rows = app.cached_rows(key, inner.width, source_len, || lines.iter().map(|l| wrapped_rows(l, avail_width)).sum());
    let visible = inner.height as usize;
    app.record_pane_metrics(key, total_rows, visible);

    let state = app.scroll_state_for(key, total_rows);
    let bottom = total_rows.saturating_sub(visible);
    let scroll = match state {
        ScrollState::Following => bottom,
        ScrollState::Anchored { row, .. } => row.min(bottom),
    };

    let widget = Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((scroll as u16, 0));
    f.render_widget(widget, inner);

    if let ScrollState::Anchored { rows_at_anchor, .. } = state {
        let appended = total_rows.saturating_sub(rows_at_anchor);
        if appended > 0 && inner.height > 0 {
            let hint = format!("{appended} new lines \u{b7} G");
            let hint_width = (hint.chars().count() as u16).min(inner.width);
            if hint_width > 0 {
                let hint_rect =
                    Rect { x: inner.x + inner.width - hint_width, y: inner.y + inner.height - 1, width: hint_width, height: 1 };
                f.render_widget(Clear, hint_rect);
                f.render_widget(Paragraph::new(hint).style(Style::default().fg(Color::DarkGray)), hint_rect);
            }
        }
    }
}

const QUEUE_STRIP_ROWS: usize = 4;

fn queue_strip_height(queued: usize) -> u16 {
    if queued == 0 {
        0
    } else {
        (1 + queued.min(QUEUE_STRIP_ROWS)) as u16
    }
}

fn draw_queue_strip(f: &mut Frame, app: &App, area: Rect) {
    if app.queue.is_empty() || area.height == 0 {
        return;
    }
    let width = area.width as usize;
    let dim = Style::default().fg(Color::DarkGray);
    let mut lines = vec![Line::from(Span::styled(format!("queued {} \u{b7} \u{2191} edits the last", app.queue.len()), dim))];
    if app.queue.len() <= QUEUE_STRIP_ROWS {
        for line in &app.queue {
            lines.push(Line::from(Span::styled(cut_to_width(line, width), dim)));
        }
    } else {
        for line in app.queue.iter().take(QUEUE_STRIP_ROWS - 1) {
            lines.push(Line::from(Span::styled(cut_to_width(line, width), dim)));
        }
        let more = app.queue.len() - (QUEUE_STRIP_ROWS - 1);
        lines.push(Line::from(Span::styled(format!("\u{2026} and {more} more"), dim)));
    }
    f.render_widget(Paragraph::new(lines), area);
}

fn draw_big_view(f: &mut Frame, app: &mut App, area: Rect) {
    let inner_width = area.width.saturating_sub(2).max(1) as usize;
    app.set_input_width(inner_width);
    let rows = crate::panes::app::wrap_input(&app.input, inner_width);
    let cap = ((area.height / 3) as usize).max(1);
    let input_height = ((rows.len() + 2).clamp(3, cap + 2)) as u16;
    let strip_height = queue_strip_height(app.queue.len());
    let parts = Layout::vertical([Constraint::Min(3), Constraint::Length(strip_height), Constraint::Length(input_height)]).split(area);

    match app.current_party() {
        PartyKey::Main(_) if app.doc.is_some() => draw_doc_pane(f, app, parts[0]),
        PartyKey::Main(idx) => draw_main_turns_pane(f, app, parts[0], idx),
        party if is_split_party(party) => draw_split_view(f, app, parts[0]),
        _ => {
            let title = party_title(app);
            let key = PaneKey(app.current_party(), Pane::Single);
            let border_style =
                if matches!(app.focus, FocusTarget::BigView(_)) { Style::default().fg(Color::Cyan) } else { Style::default() };
            let lines = session_page_lines(&app.state);
            draw_scrollable_pane(f, app, parts[0], key, lines, &title, border_style);
        }
    }

    draw_queue_strip(f, app, parts[1]);
    draw_input(f, app, parts[2], &rows);

    if app.mode == Mode::Insert && app.input == "/" {
        draw_slash_popup(f, area);
    }
}

fn pane_border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn draw_split_view(f: &mut Frame, app: &mut App, area: Rect) {
    app.ensure_trace_loaded();

    let title = party_title(app);
    let outer = Block::default().borders(Borders::ALL).title(title);
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let brief_h = (inner.height / 3).clamp(3, inner.height.saturating_sub(3).max(3));
    let halves = Layout::vertical([Constraint::Length(brief_h), Constraint::Min(3)]).split(inner);

    let party = app.current_party();
    let row = match party {
        PartyKey::Hand(i) => app.state.hands.get(i),
        PartyKey::Reviewer(i) => app.state.reviewers.get(i),
        _ => None,
    };
    let session_started = app.state.session_started;
    let big_view_focused = matches!(app.focus, FocusTarget::BigView(_));
    let brief_focused = big_view_focused && app.half_focus == Half::Brief;
    let turns_focused = big_view_focused && app.half_focus == Half::Turns;

    let brief_width = halves[0].width.saturating_sub(2);
    let brief_lines = row.map(|r| brief_pane_lines(r, session_started, brief_width)).unwrap_or_default();
    let turns_width = halves[1].width.saturating_sub(2) as usize;
    let turns_lines = row.map(|r| turns_pane_lines(r, turns_width)).unwrap_or_default();

    draw_scrollable_pane(f, app, halves[0], PaneKey(party, Pane::Brief), brief_lines, "brief", pane_border_style(brief_focused));
    draw_scrollable_pane(f, app, halves[1], PaneKey(party, Pane::Turns), turns_lines, "turns", pane_border_style(turns_focused));
}

fn draw_input(f: &mut Frame, app: &mut App, area: Rect, rows: &[crate::panes::app::Row]) {
    let insert = app.mode == Mode::Insert;
    let border_style =
        if insert { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) };
    let text_style = if insert { Style::default() } else { Style::default().fg(Color::DarkGray) };
    let block = Block::default().borders(Borders::ALL).title("input").border_style(border_style);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let visible = inner.height.max(1) as usize;
    let cursor_row_idx = crate::panes::app::cursor_row(rows, app.cursor);
    let mut top = app.input_view_top();
    if cursor_row_idx < top {
        top = cursor_row_idx;
    } else if cursor_row_idx >= top + visible {
        top = cursor_row_idx + 1 - visible;
    }
    let bottom_max = rows.len().saturating_sub(visible);
    top = top.min(bottom_max);
    app.set_input_view_top(top);

    let chars: Vec<char> = app.input.chars().collect();
    let lines: Vec<Line<'static>> = rows
        .iter()
        .skip(top)
        .take(visible)
        .map(|row| {
            let text: String = chars[row.start..row.end.min(chars.len())].iter().collect();
            Line::from(Span::styled(text, text_style))
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);

    if insert && app.modal.is_none() && !app.help_visible {
        let row = rows.get(cursor_row_idx).copied().unwrap_or(crate::panes::app::Row { start: 0, end: 0 });
        let cursor_end = app.cursor.min(chars.len()).max(row.start);
        let col: usize =
            chars[row.start..cursor_end].iter().map(|c| UnicodeWidthChar::width(*c).unwrap_or(0)).sum();
        let cursor_y = inner.y + (cursor_row_idx.saturating_sub(top)) as u16;
        let cursor_x = (inner.x + col as u16).min(inner.x + inner.width.saturating_sub(1));
        f.set_cursor_position((cursor_x, cursor_y));
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ArxMarkdownStyles;

impl tui_markdown::StyleSheet for ArxMarkdownStyles {
    fn heading(&self, _level: u8) -> Style {
        Style::default().add_modifier(Modifier::BOLD)
    }

    fn code(&self) -> Style {
        Style::default().fg(Color::Cyan)
    }

    fn table_header(&self) -> Style {
        Style::default().add_modifier(Modifier::BOLD)
    }
}

const TABLE_BORDER_CHARS: [char; 11] = ['\u{2500}', '\u{2502}', '\u{250c}', '\u{252c}', '\u{2510}', '\u{251c}', '\u{253c}', '\u{2524}', '\u{2514}', '\u{2534}', '\u{2518}'];

fn is_table_line(line: &Line) -> bool {
    line.spans.iter().any(|s| s.content.chars().any(|c| TABLE_BORDER_CHARS.contains(&c)))
}

// A joined cluster's display width is not the sum of its parts' individual widths (a
// variation selector can turn a 1-column base into a 2-column glyph, and a four-person ZWJ
// family renders as one glyph, not four) — only `UnicodeWidthStr::width` on the finished
// string gets that right, so both the fits-already check and the actual wrap route through
// the same flattened clusters rather than a separate per-span sum that a cluster split
// across two differently styled spans could still get wrong.
fn line_display_width(line: &Line) -> usize {
    flatten_line(line).iter().map(|c| c.2).sum()
}

fn joins_to_previous_cluster(ch: char) -> bool {
    ch == '\u{200D}' || ('\u{FE00}'..='\u{FE0F}').contains(&ch)
}

/// Splits a line into wrap units no narrower than one grapheme-ish cluster: a zero-width
/// joiner or variation selector glues onto whatever came before it, and a joiner also
/// glues the character right after it (so a ZWJ-joined emoji sequence stays one atomic
/// unit rather than a break landing between its parts, even across a style change).
fn flatten_line(line: &Line) -> Vec<(String, Style, usize)> {
    let mut clusters: Vec<(String, Style)> = Vec::new();
    let mut glue_next = false;
    for span in &line.spans {
        for ch in span.content.chars() {
            let glue_this = glue_next || joins_to_previous_cluster(ch);
            glue_next = ch == '\u{200D}';
            if glue_this {
                if let Some(last) = clusters.last_mut() {
                    last.0.push(ch);
                    continue;
                }
            }
            clusters.push((ch.to_string(), span.style));
        }
    }
    clusters.into_iter().map(|(text, style)| { let width = UnicodeWidthStr::width(text.as_str()); (text, style, width) }).collect()
}

fn rebuild_line(clusters: &[(String, Style, usize)]) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let mut current_style: Option<Style> = None;
    for (text, style, _) in clusters {
        if current_style != Some(*style) && !buf.is_empty() {
            spans.push(Span::styled(std::mem::take(&mut buf), current_style.expect("buf non-empty implies a style was set")));
        }
        current_style = Some(*style);
        buf.push_str(text);
    }
    if !buf.is_empty() {
        spans.push(Span::styled(buf, current_style.expect("buf non-empty implies a style was set")));
    }
    Line::from(spans)
}

/// Reflows a line to `width` columns without inserting or dropping any character the
/// source didn't already have: spans stay glued at their exact boundaries (no synthetic
/// space between a styled run and its neighbour) and a line that already fits is
/// returned untouched. Only genuinely overlong lines get walked and broken, preferably
/// at a space (which is then the one cluster a wrap consumes), falling back to a hard
/// break at the width when no space is available; the walk never splits a joined cluster.
fn wrap_line(line: Line<'static>, width: usize) -> Vec<Line<'static>> {
    let width = width.max(1);
    if is_table_line(&line) || line_display_width(&line) <= width {
        return vec![line];
    }
    let clusters = flatten_line(&line);

    let mut rows: Vec<Vec<(String, Style, usize)>> = vec![Vec::new()];
    let mut col = 0usize;
    let mut last_space: Option<usize> = None;

    for cluster in clusters {
        let cluster_width = cluster.2;
        if !rows.last().expect("at least one row").is_empty() && col + cluster_width > width {
            let row = rows.last_mut().expect("at least one row");
            let overflow = match last_space {
                Some(space_idx) => {
                    let overflow = row.split_off(space_idx + 1);
                    row.truncate(space_idx);
                    overflow
                }
                None => Vec::new(),
            };
            col = overflow.iter().map(|c| c.2).sum();
            rows.push(overflow);
            last_space = None;
        }
        if cluster.0 == " " {
            last_space = Some(rows.last().expect("at least one row").len());
        }
        rows.last_mut().expect("at least one row").push(cluster);
        col += cluster_width;
    }

    rows.into_iter().filter(|r| !r.is_empty()).map(|r| rebuild_line(&r)).collect()
}

fn render_markdown(text: &str, width: u16) -> Vec<Line<'static>> {
    let options = tui_markdown::Options::new(ArxMarkdownStyles);
    let rendered = tui_markdown::from_str_with_options(text, &options);
    let width = width.max(1) as usize;
    let mut out = Vec::new();
    for line in rendered.lines {
        let owned: Line<'static> = Line::from(
            line.spans.into_iter().map(|s| Span::styled(s.content.into_owned(), s.style)).collect::<Vec<_>>(),
        );
        out.extend(wrap_line(owned, width));
    }
    out
}

fn format_size(bytes: u64) -> String {
    if bytes < 1000 {
        format!("{bytes}b")
    } else {
        format!("{:.1}k", bytes as f64 / 1000.0)
    }
}

fn format_call_line(call: &crate::panes::state::ToolCall) -> String {
    let mut segments = vec![call.tool.clone()];
    if let Some(subject) = call.subject.as_deref() {
        if !subject.is_empty() {
            segments.push(subject.to_string());
        }
    }
    let mut outcome_bits = Vec::new();
    if let Some(outcome) = &call.outcome {
        outcome_bits.push(outcome.clone());
    }
    if let Some(bytes) = call.bytes {
        outcome_bits.push(format_size(bytes));
    }
    if !outcome_bits.is_empty() {
        segments.push(format!("\u{2192} {}", outcome_bits.join(" ")));
    }
    if let Some(handle) = &call.handle {
        let basename = std::path::Path::new(handle).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| handle.clone());
        segments.push(format!("\u{2192} {basename}"));
    }
    segments.join("  ")
}

const GUTTER_WIDTH: usize = 4;
const CURSOR_BAR: &str = "\u{258e}";

fn dashed_rule(width: u16, color: Color) -> Line<'static> {
    let width = width as usize;
    let mut text = String::from("  ");
    while text.chars().count() < width {
        text.push_str("\u{2500} ");
    }
    let text: String = text.chars().take(width).collect();
    Line::from(Span::styled(text, Style::default().fg(color)))
}

fn prefix_lines(body: Vec<Line<'static>>, glyph: Option<Span<'static>>, indent: usize, is_cursor_stop: bool, gutter_color: Color) -> Vec<Line<'static>> {
    body.into_iter()
        .enumerate()
        .map(|(i, line)| {
            let mut spans = Vec::with_capacity(line.spans.len() + 4);
            spans.push(if is_cursor_stop {
                Span::styled(CURSOR_BAR, Style::default().fg(gutter_color))
            } else {
                Span::raw(" ")
            });
            spans.push(Span::raw(" "));
            if i == 0 {
                spans.push(glyph.clone().unwrap_or_else(|| Span::raw(" ")));
            } else {
                spans.push(Span::raw(" "));
            }
            spans.push(Span::raw(" "));
            if indent > 0 {
                spans.push(Span::raw(" ".repeat(indent)));
            }
            spans.extend(line.spans);
            Line::from(spans)
        })
        .collect()
}

struct PageRender {
    lines: Vec<Line<'static>>,
    stops: Vec<(usize, usize)>,
}

fn render_main_turns(app: &mut App, idx: usize, width: u16) -> PageRender {
    let Some(main) = app.state.mains.get(idx) else {
        return PageRender { lines: Vec::new(), stops: Vec::new() };
    };
    // Cloned once so `main`'s borrow of `app.state` doesn't fight the `&mut App`
    // `cached_markdown` needs below; turn text is immutable once pushed, so this
    // costs one page-sized clone per frame instead of a full markdown re-parse.
    let ended = main.ended.clone();
    let page = main.page.clone();
    let palette = app.palette;
    let cursor = app.main_cursor_stop(idx);
    let stops = crate::panes::state::stops(&page);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut ranges: Vec<(usize, usize)> = Vec::new();

    if let Some(reason) = &ended {
        lines.push(Line::from(Span::styled(
            format!("ended: {reason}"),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
    }

    for (turn_idx, turn) in page.iter().enumerate() {
        if turn_idx > 0 {
            lines.push(dashed_rule(width, palette.rule));
        }
        match turn {
            Turn::Operator(text) => {
                let body_width = (width as usize).saturating_sub(GUTTER_WIDTH);
                let body = wrap_line(
                    Line::from(Span::styled(text.clone(), Style::default().add_modifier(Modifier::BOLD))),
                    body_width,
                );
                let is_cursor = stops.get(cursor).is_some_and(|s| s.turn == turn_idx);
                let glyph = Span::styled(">", Style::default().fg(palette.operator).add_modifier(Modifier::BOLD));
                let row_start = lines.len();
                lines.extend(prefix_lines(body, Some(glyph), 0, is_cursor, palette.gutter));
                ranges.push((row_start, lines.len()));
            }
            Turn::Main(text) => {
                let body_width = (width as usize).saturating_sub(GUTTER_WIDTH + 2);
                let body_width = body_width as u16;
                let key = crate::panes::app::MarkdownCacheKey::MainTurn { main: idx, turn: turn_idx };
                let text = text.clone();
                let body = app.cached_markdown(key, body_width, || render_markdown(&text, body_width));
                let is_cursor = stops.get(cursor).is_some_and(|s| s.turn == turn_idx);
                let row_start = lines.len();
                lines.extend(prefix_lines(body, None, 2, is_cursor, palette.gutter));
                ranges.push((row_start, lines.len()));
            }
            Turn::Notice(text) => {
                let body_width = (width as usize).saturating_sub(GUTTER_WIDTH);
                let mut body = Vec::new();
                for raw in text.split('\n') {
                    body.extend(wrap_line(
                        Line::from(Span::styled(raw.to_string(), Style::default().fg(Color::DarkGray))),
                        body_width,
                    ));
                }
                let is_cursor = stops.get(cursor).is_some_and(|s| s.turn == turn_idx);
                let row_start = lines.len();
                lines.extend(prefix_lines(body, None, 0, is_cursor, palette.gutter));
                ranges.push((row_start, lines.len()));
            }
            Turn::Tools { calls, expanded } => {
                let body_width = (width as usize).saturating_sub(GUTTER_WIDTH + 4);
                let glyph = Span::styled("\u{b7}", Style::default().fg(palette.tool));
                let tool_style = Style::default().fg(palette.tool).add_modifier(Modifier::DIM);
                if !*expanded {
                    let body = wrap_line(
                        Line::from(Span::styled(format!("{} tools used", calls.len()), tool_style)),
                        body_width,
                    );
                    let is_cursor = stops.get(cursor).is_some_and(|s| s.turn == turn_idx && s.call.is_none());
                    let row_start = lines.len();
                    lines.extend(prefix_lines(body, Some(glyph), 4, is_cursor, palette.gutter));
                    ranges.push((row_start, lines.len()));
                } else {
                    for (call_idx, call) in calls.iter().enumerate() {
                        let body = wrap_line(Line::from(Span::styled(format_call_line(call), tool_style)), body_width);
                        let is_cursor =
                            stops.get(cursor).is_some_and(|s| s.turn == turn_idx && s.call == Some(call_idx));
                        let row_start = lines.len();
                        lines.extend(prefix_lines(body, Some(glyph.clone()), 4, is_cursor, palette.gutter));
                        ranges.push((row_start, lines.len()));
                    }
                }
            }
        }
    }

    let is_live = idx == app.state.current_main && ended.is_none();
    if is_live {
        if let Some(buf) = app.state.streaming_buffer.clone() {
            if !page.is_empty() {
                lines.push(dashed_rule(width, palette.rule));
            }
            let body_width = (width as usize).saturating_sub(GUTTER_WIDTH + 2);
            let body = render_markdown(&buf, body_width as u16);
            lines.extend(prefix_lines(body, None, 2, false, palette.gutter));
        } else if app.state.is_streaming() {
            let elapsed = app.state.waiting_since.map(|s| s.elapsed().as_secs_f32()).unwrap_or(0.0);
            let frame = SPINNER[(elapsed * 4.0) as usize % SPINNER.len()];
            lines.push(Line::from(Span::styled(
                format!("{frame} thinking {elapsed:.1}s"),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    PageRender { lines, stops: ranges }
}

fn draw_main_turns_pane(f: &mut Frame, app: &mut App, area: Rect, idx: usize) {
    let title = party_title(app);
    let border_style =
        if matches!(app.focus, FocusTarget::BigView(_)) { Style::default().fg(Color::Cyan) } else { Style::default() };
    let block = Block::default().borders(Borders::ALL).title(title).border_style(border_style);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let width = inner.width.max(1);
    let render = render_main_turns(app, idx, width);
    app.record_main_stop_lines(idx, render.stops.clone());

    let visible = inner.height as usize;
    let total = render.lines.len();
    app.record_pane_metrics(PaneKey(PartyKey::Main(idx), Pane::Single), total, visible);

    let cursor = app.main_cursor_stop(idx);
    let mut offset = app.main_view_offset(idx);
    if let Some(&(start, end)) = render.stops.get(cursor) {
        let stop_height = end.saturating_sub(start);
        if stop_height >= visible {
            offset = start;
        } else if start < offset {
            offset = start;
        } else if end > offset + visible {
            offset = end.saturating_sub(visible);
        }
    }
    let bottom = total.saturating_sub(visible);
    offset = offset.min(bottom);
    app.set_main_view_offset(idx, offset);

    let widget = Paragraph::new(render.lines).scroll((offset as u16, 0));
    f.render_widget(widget, inner);
}

fn draw_doc_pane(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(doc) = app.doc.clone() else {
        return;
    };
    let key = PaneKey(PartyKey::Main(app.main_selected), Pane::Doc);
    let border_style = Style::default().fg(Color::Cyan);
    let width = area.width.saturating_sub(2).max(1) as u16;
    // A trace can be arbitrarily large (externalization exists precisely for oversized
    // results); cached so an idle doc view doesn't re-run tui-markdown every ~33ms tick.
    let body = doc.body.clone();
    let lines = app.cached_markdown(crate::panes::app::MarkdownCacheKey::Doc, width, || render_markdown(&body, width));
    draw_scrollable_pane(f, app, area, key, lines, &doc.title, border_style);
}

fn occupancy_bar(status: &Status, width: usize) -> Line<'static> {
    let width = width.max(10);
    if status.window == 0 {
        let empty: String = ".".repeat(width);
        return Line::from(Span::styled(format!("[{empty}]"), Style::default().fg(Color::DarkGray)));
    }
    let occ_idx = (((status.occupancy as f64 / status.window as f64) * width as f64).round() as usize).min(width);
    let ceiling_idx = (((status.window.saturating_sub(status.reserve)) as f64 / status.window as f64) * width as f64).round() as usize;
    let nudge_idx = ((status.nudge as f64 / status.window as f64) * width as f64).round() as usize;
    let mut chars: Vec<char> = (0..width).map(|i| if i < occ_idx { '#' } else { '.' }).collect();
    if nudge_idx < width {
        chars[nudge_idx] = ':';
    }
    if ceiling_idx < width {
        chars[ceiling_idx] = '|';
    }
    let color = if status.occupancy >= status.window.saturating_sub(status.reserve) {
        Color::LightRed
    } else if status.occupancy >= status.nudge {
        Color::Yellow
    } else {
        Color::Green
    };
    let bar: String = chars.into_iter().collect();
    Line::from(Span::styled(format!("[{bar}]"), Style::default().fg(color)))
}

fn cell(text: &str, width: usize) -> String {
    let mut out: String = text.chars().take(width).collect();
    while out.chars().count() < width {
        out.push(' ');
    }
    out
}

/// Fits one status-terminated row into `width` cells without ever letting it wrap: the
/// status cell is never touched, and `prefix` is cut from the right only if the row is
/// still too wide once cell(s) a caller already dropped are gone.
fn fit_row_to_width(prefix: &str, status: &str, width: usize) -> String {
    let width = width.max(1);
    let full = format!("{prefix}  {status}");
    if UnicodeWidthStr::width(full.as_str()) <= width {
        return full;
    }
    let status_width = UnicodeWidthStr::width(status);
    let budget = width.saturating_sub(status_width + 2);
    format!("{}  {status}", cut_to_width(prefix, budget))
}

fn main_row_text(label: &str, model: &str, status: &str, width: usize) -> String {
    let width = width.max(1);
    let full = format!("{label}  {model}  {status}");
    if UnicodeWidthStr::width(full.as_str()) <= width {
        return full;
    }
    fit_row_to_width(label, status, width)
}

fn panel_block(id: PanelId, active: bool) -> Block<'static> {
    let title = format!("{} {}", id.number(), id.name());
    let style = if active { Style::default().fg(Color::Cyan) } else { Style::default() };
    Block::default().borders(Borders::ALL).title(title).border_style(style)
}

fn draw_right_column(f: &mut Frame, app: &App, area: Rect) -> [Rect; 4] {
    let main_h = (app.state.mains.len() as u16 + 2).max(3);
    let reviewers_h = (app.state.reviewers.len() as u16 + 2).max(3);
    let parts = Layout::vertical([
        Constraint::Length(7),
        Constraint::Length(main_h),
        Constraint::Length(reviewers_h),
        Constraint::Min(3),
    ])
    .split(area);

    draw_session_panel(f, app, parts[0]);
    draw_main_panel(f, app, parts[1]);
    draw_reviewers_panel(f, app, parts[2]);
    draw_hands_panel(f, app, parts[3]);
    [parts[0], parts[1], parts[2], parts[3]]
}

fn draw_session_panel(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel() == PanelId::Session;
    let s = &app.state.status;
    let lines = vec![
        Line::from(format!("model  {}", s.model)),
        occupancy_bar(s, (area.width as usize).saturating_sub(4).max(10)),
        Line::from(format!("{} / {}", s.occupancy, s.window)),
        Line::from(format!("reserve {}  cp {}", s.reserve, s.checkpoint.map(|(n, _)| n).unwrap_or(0))),
        Line::from(format!("gate allow {} deny {}", app.state.gate.allow, app.state.gate.deny)),
    ];
    f.render_widget(Paragraph::new(lines).block(panel_block(PanelId::Session, active)).wrap(Wrap { trim: false }), area);
}

fn draw_main_panel(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel() == PanelId::Main;
    let width = area.width.saturating_sub(2) as usize;
    let mut lines = Vec::new();
    for (i, m) in app.state.mains.iter().enumerate().rev() {
        let status = m.status_cell();
        let text = main_row_text(&m.label, &m.model, &status, width);
        let selected = i == app.main_selected;
        let style = if selected { Style::default().fg(Color::Black).bg(Color::Cyan) } else { Style::default() };
        lines.push(Line::from(Span::styled(text, style)));
    }
    f.render_widget(Paragraph::new(lines).block(panel_block(PanelId::Main, active)), area);
}

fn draw_reviewers_panel(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel() == PanelId::Reviewers;
    draw_party_list(f, PanelId::Reviewers, active, &app.state.reviewers, app.reviewer_selected, area);
}

fn draw_hands_panel(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel() == PanelId::Hands;
    draw_party_list(f, PanelId::Hands, active, &app.state.hands, app.hand_selected, area);
}

fn draw_party_list(f: &mut Frame, id: PanelId, active: bool, rows: &[HandRow], selected: usize, area: Rect) {
    let width = area.width.saturating_sub(2) as usize;
    let drop_agent = width < 24;
    let mut lines = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let status = row.status_cell();
        let prefix = if drop_agent { cell(&row.id, 7) } else { format!("{}  {}", cell(&row.id, 7), cell(&row.agent, 11)) };
        let text = fit_row_to_width(&prefix, &status, width);
        let style = if i == selected { Style::default().fg(Color::Black).bg(Color::Cyan) } else { Style::default() };
        lines.push(Line::from(Span::styled(text, style)));
    }
    f.render_widget(Paragraph::new(lines).block(panel_block(id, active)), area);
}

fn primary<'a>(table: &'a KeyTable, action: &str) -> &'a str {
    table.primary(action).unwrap_or("?")
}

fn draw_hint_bar(f: &mut Frame, app: &App, area: Rect) {
    if let Some(hint) = &app.transient_hint {
        f.render_widget(Paragraph::new(format!(" {hint}")).style(Style::default().fg(Color::Yellow)), area);
        return;
    }
    if matches!(app.focus, FocusTarget::BigView(_)) && is_split_party(app.current_party()) {
        draw_split_hint_bar(f, app, area);
        return;
    }

    let mode_label = match app.mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
    };
    let panel = app.active_panel();
    let mut hints: Vec<String> = Vec::new();
    match app.mode {
        Mode::Insert => {
            hints.push(format!("{} submit", primary(&app.keys.insert, "submit")));
            hints.push(format!("{} normal", primary(&app.keys.insert, "normal")));
            hints.push(format!("{} newline", primary(&app.keys.insert, "newline")));
            hints.push(format!("{} history", primary(&app.keys.insert, "history_up")));
            hints.push(format!("{} complete", primary(&app.keys.insert, "complete")));
        }
        Mode::Normal => {
            hints.push(format!("{} insert", primary(&app.keys.normal, "insert")));
            hints.push("1-4 panels".to_string());
            match app.focus {
                FocusTarget::Panel(_) => {
                    hints.push(format!(
                        "{}/{} move",
                        primary(&app.keys.normal, "up"),
                        primary(&app.keys.normal, "down")
                    ));
                    hints.push(format!("{} open", primary(&app.keys.normal, "open")));
                }
                FocusTarget::BigView(_) => {
                    hints.push(format!(
                        "{}/{} scroll",
                        primary(&app.keys.normal, "up"),
                        primary(&app.keys.normal, "down")
                    ));
                    hints.push(format!("{} back", primary(&app.keys.normal, "back")));
                }
            }
            hints.push(format!("{} keys", primary(&app.keys.normal, "help")));
            hints.push(format!("{} quit", primary(&app.keys.normal, "quit")));
        }
    }
    let line = format!(" {mode_label}  {} {}   {}", panel.number(), panel.name(), hints.join(" \u{b7} "));
    f.render_widget(Paragraph::new(line).style(Style::default().fg(Color::DarkGray)), area);
}

fn draw_split_hint_bar(f: &mut Frame, app: &App, area: Rect) {
    let mode_label = match app.mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
    };
    let panel = app.active_panel();
    let swap = primary(&app.keys.split, "swap_pane");
    let down = primary(&app.keys.normal, "down");
    let up = primary(&app.keys.normal, "up");
    let top = primary(&app.keys.normal, "top");
    let bottom = primary(&app.keys.normal, "bottom");
    let back = primary(&app.keys.normal, "back");
    let help = primary(&app.keys.normal, "help");
    let line = format!(
        " {mode_label}  {} {} \u{b7} {}   {swap} pane \u{b7} {down}/{up} \u{b7} {top}{top}/{bottom} \u{b7} {back} back \u{b7} {help} keys",
        panel.number(),
        panel.name(),
        party_short(app),
    );
    f.render_widget(Paragraph::new(line).style(Style::default().fg(Color::DarkGray)), area);
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect { x, y, width, height }
}

fn draw_slash_popup(f: &mut Frame, area: Rect) {
    let commands = ["/reset", "/model", "/model <id>", "/model <position> <id>", "/model clear <position>", "/checkpoint"];
    let rect = Rect {
        x: area.x + 2,
        y: area.y.saturating_add(area.height.saturating_sub(3 + commands.len() as u16)),
        width: 40.min(area.width.saturating_sub(4)),
        height: 2 + commands.len() as u16,
    };
    let lines: Vec<Line> = commands.iter().map(|c| Line::from(*c)).collect();
    let block = Block::default().borders(Borders::ALL).title("commands");
    f.render_widget(Clear, rect);
    f.render_widget(Paragraph::new(lines).block(block), rect);
}

fn model_picker_selected_family<'a>(models: &'a [RosterModel], tab: &ModelPickerTab) -> Option<&'a str> {
    let id = if tab.cursor == 0 {
        tab.fallback_id.as_str()
    } else {
        tab.model_ids.get(tab.cursor - 1)?.as_str()
    };
    models.iter().find(|m| m.id == id).map(|m| m.family.as_str())
}

fn draw_model_picker(f: &mut Frame, area: Rect, models: &[RosterModel], tabs: &[ModelPickerTab], tab: usize, main_family: &str) {
    let Some(active) = tabs.get(tab) else { return };
    let warns = active.position == "reviewer" && model_picker_selected_family(models, active) == Some(main_family);
    let height = (active.model_ids.len() as u16) + 5 + if warns { 1 } else { 0 };
    let rect = centered(area, 44, height);
    f.render_widget(Clear, rect);

    let mut strip = String::new();
    for (i, t) in tabs.iter().enumerate() {
        if i > 0 {
            strip.push(' ');
        }
        if i == tab {
            strip.push_str(&format!("[{}]", t.position));
        } else {
            strip.push_str(&t.position);
        }
    }
    let mut lines = vec![Line::from(Span::styled(strip, Style::default().add_modifier(Modifier::BOLD))), Line::from("")];

    let from_file_marker = if active.cursor == 0 { "\u{25b8} " } else { "  " };
    let from_file_style = if active.cursor == 0 { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default() };
    lines.push(Line::from(Span::styled(
        format!("{from_file_marker}from file ({})", active.fallback_id),
        from_file_style,
    )));
    for (i, id) in active.model_ids.iter().enumerate() {
        let Some(m) = models.iter().find(|m| &m.id == id) else { continue };
        let cursor = i + 1;
        let marker = if cursor == active.cursor { "\u{25b8} " } else { "  " };
        let style = if cursor == active.cursor { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default() };
        lines.push(Line::from(Span::styled(format!("{marker}{cursor}. {} ({})", m.id, m.body), style)));
    }
    if warns {
        lines.push(Line::from(Span::styled(
            "warning: this pin shares main's model family — its verdict would not be independent",
            Style::default().fg(Color::Yellow),
        )));
    }
    lines.push(Line::from(Span::styled(
        "h/l tabs \u{b7} j/k or arrows, digits, Enter, Esc",
        Style::default().fg(Color::DarkGray),
    )));
    let block = Block::default().borders(Borders::ALL).title("/model").border_style(Style::default().fg(Color::Cyan));
    f.render_widget(Paragraph::new(lines).block(block), rect);
}

fn draw_confirm_modal(f: &mut Frame, area: Rect, text: &str, input: &str, action: &crate::panes::app::ConfirmAction) {
    let rect = centered(area, 74, 8);
    f.render_widget(Clear, rect);
    let mut lines = vec![Line::from(text.to_string()), Line::from("")];
    let (title, hint) = match action {
        crate::panes::app::ConfirmAction::Reset => {
            lines.push(Line::from(format!("> {input}")));
            ("reset", "Enter ok \u{b7} type a note then Enter \u{b7} Esc sends nothing")
        }
        crate::panes::app::ConfirmAction::Kill { .. } => ("kill", "Enter kills, Esc keeps"),
    };
    lines.push(Line::from(Span::styled(hint, Style::default().fg(Color::DarkGray))));
    let block = Block::default().borders(Borders::ALL).title(title).border_style(Style::default().fg(Color::Cyan));
    f.render_widget(Paragraph::new(lines).block(block).wrap(Wrap { trim: false }), rect);
}

fn draw_help_overlay(f: &mut Frame, area: Rect, keys: &KeyMap) {
    let rect = centered(area, 74, area.height.saturating_sub(2).min(26));
    f.render_widget(Clear, rect);
    let mut lines = vec![Line::from(Span::styled("keys (from keys.toml)", Style::default().add_modifier(Modifier::BOLD))), Line::from("")];
    let groups: [(&str, &[(&str, &str)], &KeyTable); 4] = [
        ("normal", &NORMAL_ACTIONS, &keys.normal),
        ("insert", &INSERT_ACTIONS, &keys.insert),
        ("modal", &MODAL_ACTIONS, &keys.modal),
        ("split", &SPLIT_ACTIONS, &keys.split),
    ];
    for (label, actions, table) in groups {
        lines.push(Line::from(Span::styled(label, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))));
        for (action, desc) in actions {
            let bound = table.bound(action).join(", ");
            lines.push(Line::from(format!("  {action:<12} {bound:<20} {desc}")));
        }
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled("any key closes this", Style::default().fg(Color::DarkGray))));
    let block = Block::default().borders(Borders::ALL).title("? keys").border_style(Style::default().fg(Color::Cyan));
    f.render_widget(Paragraph::new(lines).block(block).wrap(Wrap { trim: false }), rect);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use crate::events::Event;
    use crate::terminal::TerminalMsg;

    fn frame_text(terminal: &Terminal<TestBackend>) -> String {
        let buffer = terminal.backend().buffer();
        let area = buffer.area;
        let mut out = String::new();
        for y in 0..area.height {
            let mut row = String::new();
            for x in 0..area.width {
                row.push_str(buffer[(x, y)].symbol());
            }
            out.push_str(row.trim_end());
            out.push('\n');
        }
        out
    }

    #[test]
    fn a_scripted_message_sequence_renders_the_expected_frames() {
        let backend = TestBackend::new(110, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(None);

        app.apply_terminal(TerminalMsg::Prompt);
        app.submit_line("hello".to_string());
        app.apply_terminal(TerminalMsg::Chunk { text: "stub".to_string() });
        app.apply_terminal(TerminalMsg::Chunk { text: ":hello".to_string() });
        app.apply_terminal(TerminalMsg::TurnEnd { text: "stub:hello".to_string() });
        app.apply_terminal(TerminalMsg::Status {
            model: "stub-model".to_string(),
            occupancy: 1234,
            window: 200_000,
            reserve: 16_000,
            nudge: 120_000,
            checkpoint: None,
            turn: 1,
        });
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let frame = frame_text(&terminal);
        assert!(frame.lines().next().unwrap().contains("main #1"), "frame:\n{frame}");
        assert!(frame.contains("> hello"), "frame:\n{frame}");
        assert!(frame.contains("stub:hello"), "frame:\n{frame}");
        assert!(frame.contains("NORMAL"), "frame:\n{frame}");
        assert_eq!(app.outbox.pop_front(), Some("hello".to_string()));

        app.apply_event(Event::Spawn { hand: "h-0001".to_string(), agent: "hand-do".to_string(), role: "hand".to_string() });
        app.apply_event(Event::Delivered {
            hand: "h-0001".to_string(),
            outcome: "verdict".to_string(),
            trace: Some("threads/main.traces/h-0001".to_string()),
            bytes: Some(512),
            elapsed_ms: Some(4200),
            lag_ms: Some(30),
        });
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let hands_frame = frame_text(&terminal);
        assert!(hands_frame.contains("4 hands"), "frame:\n{hands_frame}");
        assert!(hands_frame.contains("h-0001"), "frame:\n{hands_frame}");
    }

    #[test]
    fn the_content_fingerprint_changes_when_a_single_line_grows_even_though_the_line_count_does_not() {
        let short = vec![Line::from("stub:")];
        let longer = vec![Line::from("stub: a much longer streamed chunk of text")];
        assert_eq!(short.len(), longer.len());
        assert_ne!(content_fingerprint(&short), content_fingerprint(&longer));
        assert_eq!(content_fingerprint(&short), content_fingerprint(&short));
    }

    #[test]
    fn the_content_fingerprint_distinguishes_equal_total_length_content_redistributed_across_lines() {
        let one_line = vec![Line::from("abcdef")];
        let two_lines = vec![Line::from("abc"), Line::from("def")];
        let reordered = vec![Line::from("defabc")];
        assert_ne!(content_fingerprint(&one_line), content_fingerprint(&two_lines));
        assert_ne!(content_fingerprint(&one_line), content_fingerprint(&reordered));
    }

    fn spawn_hand(app: &mut App, hand: &str, body: &str, brief: &str) {
        app.apply_event(Event::Spawn { hand: hand.to_string(), agent: "hand-do".to_string(), role: "hand".to_string() });
        app.apply_terminal(TerminalMsg::HandSpawned {
            hand: hand.to_string(),
            agent: "hand-do".to_string(),
            role: "hand".to_string(),
            body: body.to_string(),
            image: "alpine:3.20".to_string(),
            model: "fake-model".to_string(),
            brief: brief.to_string(),
        });
    }

    #[test]
    fn split_page_renders_running_stopped_and_delivered_states_with_real_trace_turns() {
        let backend = TestBackend::new(110, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(None);

        let trace_dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(trace_dir.path().join("out")).unwrap();
        std::fs::write(trace_dir.path().join("out/stdout.log"), "verdict:done: the brief\n").unwrap();

        spawn_hand(&mut app, "h-0001", "fake", "delete pid.txt and drop its mention from CLAUDE.md");
        app.focus_panel(PanelId::Hands);
        app.open_big_view();

        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let running_frame = frame_text(&terminal);
        assert!(running_frame.contains("brief"), "frame:\n{running_frame}");
        assert!(running_frame.contains("h-0001"), "frame:\n{running_frame}");
        assert!(running_frame.contains("hand-do"), "frame:\n{running_frame}");
        assert!(running_frame.contains("delete pid.txt"), "frame:\n{running_frame}");
        assert!(running_frame.contains("running"), "frame:\n{running_frame}");
        assert!(running_frame.contains("Tab pane"), "frame:\n{running_frame}");

        app.apply_terminal(TerminalMsg::HandStopped { hand: "h-0001".to_string(), question: "which db?".to_string() });
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let stopped_frame = frame_text(&terminal);
        assert!(stopped_frame.contains("stopped"), "frame:\n{stopped_frame}");
        assert!(stopped_frame.contains("which db?"), "frame:\n{stopped_frame}");

        app.apply_terminal(TerminalMsg::HandDelivered {
            hand: "h-0001".to_string(),
            outcome: "verdict".to_string(),
            text: Some("done: the brief".to_string()),
            trace: Some(trace_dir.path().display().to_string()),
            bytes: Some(512),
            elapsed_ms: 4200,
            lag_ms: 30,
            schema: None,
            diff: None,
        });
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let delivered_frame = frame_text(&terminal);
        assert!(delivered_frame.contains("verdict:done: the brief"), "frame:\n{delivered_frame}");
        assert!(delivered_frame.contains("outcome"), "frame:\n{delivered_frame}");
        assert!(delivered_frame.contains("trace"), "frame:\n{delivered_frame}");
        assert!(delivered_frame.contains("elapsed"), "frame:\n{delivered_frame}");
        assert!(!delivered_frame.contains("schema"), "frame:\n{delivered_frame}");

        app.handle_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.half_focus, Half::Brief);
        terminal.draw(|f| draw(f, &mut app)).unwrap();

        app.handle_key(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(app.focus, FocusTarget::Panel(PanelId::Hands));
    }

    #[test]
    fn an_unreadable_trace_path_renders_the_message_above_the_verdict_block() {
        let backend = TestBackend::new(110, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(None);

        spawn_hand(&mut app, "h-0002", "fake", "a brief");
        app.apply_terminal(TerminalMsg::HandDelivered {
            hand: "h-0002".to_string(),
            outcome: "verdict".to_string(),
            text: Some("done".to_string()),
            trace: Some("/does/not/exist/threads/main.h-0002".to_string()),
            bytes: Some(10),
            elapsed_ms: 100,
            lag_ms: 5,
            schema: None,
            diff: None,
        });
        app.focus_panel(PanelId::Hands);
        app.open_big_view();

        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let frame = frame_text(&terminal);
        assert!(frame.contains("trace unreadable:"), "frame:\n{frame}");
        assert!(frame.contains("outcome"), "frame:\n{frame}");
    }

    #[test]
    fn a_reviewer_s_delivered_page_shows_the_schema_line() {
        let backend = TestBackend::new(110, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(None);

        let trace_dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(trace_dir.path().join("out")).unwrap();
        std::fs::write(trace_dir.path().join("out/stdout.log"), "verdict:{}\n").unwrap();

        app.apply_event(Event::Spawn { hand: "h-0003".to_string(), agent: "codex-reviewer".to_string(), role: "reviewer".to_string() });
        app.apply_terminal(TerminalMsg::HandSpawned {
            hand: "h-0003".to_string(),
            agent: "codex-reviewer".to_string(),
            role: "reviewer".to_string(),
            body: "fake".to_string(),
            image: "alpine:3.20".to_string(),
            model: "fake-model".to_string(),
            brief: "review the diff".to_string(),
        });
        app.apply_terminal(TerminalMsg::HandDelivered {
            hand: "h-0003".to_string(),
            outcome: "verdict".to_string(),
            text: Some("{}".to_string()),
            trace: Some(trace_dir.path().display().to_string()),
            bytes: Some(2),
            elapsed_ms: 50,
            lag_ms: 1,
            schema: Some("malformed".to_string()),
            diff: None,
        });
        app.focus_panel(PanelId::Reviewers);
        app.open_big_view();

        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let frame = frame_text(&terminal);
        assert!(frame.contains("malformed"), "frame:\n{frame}");

        app.back_to_panel();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let panel_frame = frame_text(&terminal);
        assert!(panel_frame.contains("\u{2717}"), "frame:\n{panel_frame}");
        assert!(!panel_frame.contains("\u{2713}"), "frame:\n{panel_frame}");
    }

    #[test]
    fn a_growing_pane_scrolled_up_shows_the_new_lines_hint() {
        // main's own big view now derives its offset from the stop cursor (scope 3), never
        // this ScrollState hint; the session page still uses it, so this proves the mechanism
        // that split pages and the trace document (scope 6) still rely on.
        let backend = TestBackend::new(110, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(None);

        app.state.last_models = vec!["m0".to_string()];
        app.focus = FocusTarget::BigView(PanelId::Session);
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        app.handle_key(KeyCode::Char('k'), KeyModifiers::NONE);

        app.state.last_models = (0..40).map(|i| format!("model-{i}")).collect();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let frame = frame_text(&terminal);
        assert!(frame.contains("new lines \u{b7} G"), "frame:\n{frame}");
    }

    fn folded_tools(calls: &[&str]) -> Turn {
        Turn::Tools {
            calls: calls
                .iter()
                .map(|tool| crate::panes::state::ToolCall { tool: tool.to_string(), subject: None, outcome: None, bytes: None, handle: None })
                .collect(),
            expanded: false,
        }
    }

    fn bar_lines(frame: &str) -> Vec<&str> {
        frame.lines().filter(|l| l.contains(CURSOR_BAR)).collect()
    }

    #[test]
    fn the_cursor_bar_marks_only_the_cursor_s_stop_with_the_rule_and_fold_shown() {
        let backend = TestBackend::new(110, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(None);
        app.state.mains[0].page =
            vec![Turn::Operator("hi there".to_string()), Turn::Main("hello back".to_string()), folded_tools(&["read_file", "list_files"])];
        app.focus = FocusTarget::BigView(PanelId::Main);
        app.nav_up();
        assert_eq!(app.main_cursor_stop(0), 1, "cursor sits on the main turn");

        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let frame = frame_text(&terminal);
        assert!(frame.contains("> hi there"), "frame:\n{frame}");
        assert!(frame.contains("2 tools used"), "frame:\n{frame}");
        assert!(frame.contains("\u{2500} \u{2500} \u{2500}"), "no dashed rule:\n{frame}");

        let bars = bar_lines(&frame);
        assert!(!bars.is_empty(), "frame:\n{frame}");
        assert!(bars.iter().any(|l| l.contains("hello back")), "bars: {bars:?}");
        assert!(!bars.iter().any(|l| l.contains("hi there")), "bars: {bars:?}");
        assert!(!bars.iter().any(|l| l.contains("tools used")), "bars: {bars:?}");
    }

    #[test]
    fn expanding_a_fold_moves_the_bar_to_the_cursor_s_own_call_line() {
        let backend = TestBackend::new(110, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(None);
        app.state.mains[0].page = vec![Turn::Operator("hi".to_string()), folded_tools(&["read_file", "list_files"])];
        app.focus = FocusTarget::BigView(PanelId::Main);
        app.handle_key(KeyCode::Enter, KeyModifiers::NONE);
        assert!(matches!(&app.state.mains[0].page[1], Turn::Tools { expanded: true, .. }));
        app.nav_down();
        assert_eq!(app.main_cursor_stop(0), 2, "cursor on the second call");

        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let frame = frame_text(&terminal);
        assert!(frame.contains("read_file"), "frame:\n{frame}");
        assert!(frame.contains("list_files"), "frame:\n{frame}");

        let bars = bar_lines(&frame);
        assert!(bars.iter().any(|l| l.contains("list_files")), "bars: {bars:?}");
        assert!(!bars.iter().any(|l| l.contains("read_file")), "bars: {bars:?}");
    }

    const MARKDOWN_FIXTURE: &str = "## Repo root\n\nTwo crates, **one** binary.\n\n- crates/arx\n  - src/main.rs, `~4980` lines\n  - src/panes/\n- crates/arx-engine\n\n| crate | role |\n|---|---|\n| arx | the binary |\n| arx-engine | the store |\n\n```rust\nfn main() {}\n```\n";

    #[test]
    fn the_markdown_fixture_renders_a_bordered_table_and_a_bold_heading_at_width_60() {
        let backend = TestBackend::new(110, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(None);
        app.state.mains[0].page = vec![Turn::Main(MARKDOWN_FIXTURE.to_string())];
        app.focus = FocusTarget::BigView(PanelId::Main);

        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let frame = frame_text(&terminal);
        assert!(frame.contains("Repo root"), "frame:\n{frame}");
        assert!(frame.contains("\u{250c}") && frame.contains("\u{2510}"), "no top table border:\n{frame}");
        assert!(frame.contains("\u{2514}") && frame.contains("\u{2518}"), "no bottom table border:\n{frame}");
        assert!(frame.contains("crate") && frame.contains("role"), "frame:\n{frame}");
        assert!(frame.contains("arx-engine"), "frame:\n{frame}");
        assert!(frame.contains("fn main"), "frame:\n{frame}");

        let list_line = frame.lines().find(|l| l.contains("src/main.rs")).expect("nested list line present");
        let dash_col = list_line.find('-').expect("a dash marker on the nested item");
        let top_level =
            frame.lines().find(|l| l.contains("- crates/arx") && !l.contains("src") && !l.contains("engine")).expect("top-level item");
        let top_dash_col = top_level.find('-').expect("a dash marker on the top-level item");
        assert!(dash_col > top_dash_col, "nested item did not indent past the top-level one:\nlist: {list_line}\ntop: {top_level}");
    }

    #[test]
    fn a_plain_paragraph_longer_than_the_width_wraps_under_the_indent_not_at_column_0() {
        let long = "x".repeat(20);
        let paragraph = format!("{long} {long} {long} {long}");
        let lines = render_markdown(&paragraph, 20);
        assert!(lines.len() > 1, "expected the paragraph to wrap: {lines:?}");
        for line in &lines {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(!text.starts_with(' '), "a wrapped body line should not carry its own leading space: {text:?}");
        }
    }

    #[test]
    fn wrapping_never_inserts_a_space_at_a_styled_span_boundary() {
        let lines = render_markdown("**word**! and a `code`, comma", 200);
        let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "word! and a code, comma", "wrap_line must not glue words with a synthetic space");
    }

    #[test]
    fn wrapping_a_fenced_code_line_preserves_internal_whitespace() {
        let source = "```\nif x:\n    if y:\n        do_thing()\n```\n";
        let lines = render_markdown(source, 200);
        let code_line = lines.iter().find(|l| l.spans.iter().any(|s| s.content.contains("do_thing"))).expect("code line present");
        let text: String = code_line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "        do_thing()", "nested indentation inside a fenced block must survive untouched: {text:?}");
    }

    #[test]
    fn a_zwj_joined_emoji_sequence_never_splits_across_a_wrap_boundary() {
        let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";
        let text = format!("abc {family} some more words after it to force a wrap");
        let lines = wrap_line(Line::from(text), 10);
        let joined: String = lines.iter().flat_map(|l| l.spans.iter()).map(|s| s.content.as_ref()).collect();
        assert!(joined.contains(family), "the joined sequence must survive intact somewhere in the wrapped output: {lines:?}");
        for line in &lines {
            let row_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(
                !row_text.contains('\u{200D}') || row_text.contains(family),
                "a row holding part of the joiner sequence must hold all of it: {row_text:?}"
            );
        }
    }

    #[test]
    fn a_joined_cluster_s_width_is_the_glyph_s_own_display_width_not_the_sum_of_its_parts() {
        let clusters = flatten_line(&Line::from("*\u{FE0F} plain"));
        let keycap = &clusters[0];
        assert_eq!(keycap.0, "*\u{FE0F}");
        assert_eq!(keycap.2, 2, "a variation selector can widen its base to 2 columns, not the 1 its parts sum to");

        let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";
        let clusters = flatten_line(&Line::from(family.to_string()));
        assert_eq!(clusters.len(), 1, "the whole ZWJ chain must collapse into one cluster");
        assert_eq!(clusters[0].2, 2, "a joined family renders as one glyph, not the sum of four emoji widths");
    }

    #[test]
    fn line_display_width_stays_accurate_when_a_cluster_straddles_a_styled_span_boundary() {
        // "*" bold, then the variation selector plain: two spans, one glyph.
        let line = Line::from(vec![
            Span::styled("*".to_string(), Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("\u{FE0F}".to_string()),
        ]);
        assert_eq!(line_display_width(&line), 2, "a per-span width sum would undercount this as 1");

        let wrapped = wrap_line(line, 1);
        assert_eq!(wrapped.len(), 1, "width 1 cannot fit a 2-column cluster on its own row, so it must not be split");
    }

    #[test]
    fn resizing_the_main_page_re_renders_without_a_panic_or_stale_lines() {
        let backend = TestBackend::new(110, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(None);
        app.state.mains[0].page = vec![Turn::Operator("hi".to_string()), Turn::Main(MARKDOWN_FIXTURE.to_string())];
        app.focus = FocusTarget::BigView(PanelId::Main);
        terminal.draw(|f| draw(f, &mut app)).unwrap();

        terminal.resize(Rect::new(0, 0, 80, 36));
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let frame = frame_text(&terminal);
        assert!(frame.contains("Repo root"), "frame:\n{frame}");
        assert!(frame.contains("hi"), "frame:\n{frame}");
    }

    #[test]
    fn enter_on_an_externalized_call_opens_its_trace_as_a_document_and_esc_restores_the_page() {
        let dir = tempfile::tempdir().unwrap();
        let handle = dir.path().join("trace.md");
        std::fs::write(&handle, "# a trace\n\nsome findings\n").unwrap();

        let mut app = App::new(None);
        app.state.mains[0].page = vec![Turn::Tools {
            calls: vec![crate::panes::state::ToolCall {
                tool: "read_file".to_string(),
                subject: Some("src/x.rs".to_string()),
                outcome: Some("externalized".to_string()),
                bytes: Some(4096),
                handle: Some(handle.display().to_string()),
            }],
            expanded: true,
        }];
        app.focus = FocusTarget::BigView(PanelId::Main);
        assert_eq!(app.main_cursor_stop(0), 0);

        app.handle_key(KeyCode::Enter, KeyModifiers::NONE);
        let doc = app.doc.clone().expect("doc opened");
        assert_eq!(doc.title, handle.display().to_string());
        assert!(doc.body.contains("some findings"), "doc: {doc:?}");

        let backend = TestBackend::new(110, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let frame = frame_text(&terminal);
        assert!(frame.contains("a trace"), "frame:\n{frame}");
        assert!(frame.contains("some findings"), "frame:\n{frame}");

        app.handle_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.doc.is_none());
        assert_eq!(app.main_cursor_stop(0), 0, "cursor untouched by the round trip");
        assert_eq!(app.focus, FocusTarget::BigView(PanelId::Main));
    }

    #[test]
    fn a_handle_to_a_missing_file_pushes_a_notice_and_leaves_doc_none() {
        let mut app = App::new(None);
        app.state.mains[0].page = vec![Turn::Tools {
            calls: vec![crate::panes::state::ToolCall {
                tool: "read_file".to_string(),
                subject: None,
                outcome: Some("externalized".to_string()),
                bytes: Some(10),
                handle: Some("/does/not/exist/trace.md".to_string()),
            }],
            expanded: true,
        }];
        app.focus = FocusTarget::BigView(PanelId::Main);

        app.handle_key(KeyCode::Enter, KeyModifiers::NONE);

        assert!(app.doc.is_none());
        assert!(
            matches!(app.state.mains[0].page.last(), Some(Turn::Notice(text)) if text.contains("trace unreadable")),
            "page: {:?}",
            app.state.mains[0].page
        );
    }

    fn big_view_geometry(size: Rect) -> (usize, usize) {
        let root = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(size);
        let cols = Layout::horizontal([Constraint::Percentage(66), Constraint::Percentage(34)]).split(root[0]);
        let area = cols[0];
        let inner_width = area.width.saturating_sub(2).max(1) as usize;
        let cap = ((area.height / 3) as usize).max(1);
        (inner_width, cap)
    }

    #[test]
    fn a_long_input_draws_a_box_of_the_right_height_with_the_cursor_after_the_last_char() {
        let size = Rect::new(0, 0, 110, 36);
        let (inner_width, _cap) = big_view_geometry(size);
        let text = "a".repeat(300);
        let expected_rows = crate::panes::app::wrap_input(&text, inner_width);

        let backend = TestBackend::new(110, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(None);
        app.mode = Mode::Insert;
        for c in text.chars() {
            app.insert_char(c);
        }

        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let frame = frame_text(&terminal);
        let input_top = frame.lines().position(|l| l.contains("input")).expect("input box title present");
        let input_bottom = frame.lines().skip(input_top + 1).position(|l| l.trim_start().starts_with('\u{2514}')).expect("input box bottom border") + input_top + 1;
        let visible_rows = input_bottom - input_top - 1;
        assert_eq!(visible_rows, expected_rows.len(), "frame:\n{frame}");

        let cursor = terminal.backend().cursor_position();
        assert_eq!(cursor.y as usize, input_bottom - 1, "cursor must sit on the last drawn row");
        let last_row = expected_rows.last().unwrap();
        let last_row_len = last_row.end - last_row.start;
        assert_eq!(cursor.x as usize, 1 + last_row_len, "cursor must land right after the last char");
    }

    #[test]
    fn an_input_past_the_third_of_the_screen_caps_and_keeps_the_cursor_row_visible() {
        let size = Rect::new(0, 0, 110, 36);
        let (inner_width, cap) = big_view_geometry(size);
        let text = "a".repeat(2000);
        let expected_rows = crate::panes::app::wrap_input(&text, inner_width);
        assert!(expected_rows.len() > cap, "fixture must actually exceed the cap to exercise it");

        let backend = TestBackend::new(110, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(None);
        app.mode = Mode::Insert;
        for c in text.chars() {
            app.insert_char(c);
        }

        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let frame = frame_text(&terminal);
        let input_top = frame.lines().position(|l| l.contains("input")).expect("input box title present");
        let input_bottom = frame.lines().skip(input_top + 1).position(|l| l.trim_start().starts_with('\u{2514}')).expect("input box bottom border") + input_top + 1;
        let visible_rows = input_bottom - input_top - 1;
        assert_eq!(visible_rows, cap, "frame:\n{frame}");

        let cursor = terminal.backend().cursor_position();
        assert!(cursor.y as usize >= input_top + 1 && (cursor.y as usize) < input_bottom, "cursor row must stay inside the capped box");
    }

    #[test]
    fn a_wide_main_row_never_wraps_and_the_model_cell_returns_once_the_panel_is_wide_enough() {
        let mut app = App::new(None);
        app.state.apply_event(&Event::Reset { phase: "respawned".to_string(), check: None });
        app.state.mains[1].model = "opus-5-xl".to_string();
        app.state.apply_terminal(&TerminalMsg::Usage { input: 900_000, output: 100_000, cache_creation: 0, cache_read: 0 });
        app.state.push_operator_line("hi");

        let narrow_backend = TestBackend::new(110, 36);
        let mut narrow = Terminal::new(narrow_backend).unwrap();
        narrow.draw(|f| draw(f, &mut app)).unwrap();
        let narrow_frame = frame_text(&narrow);
        for line in narrow_frame.lines() {
            assert!(!line.contains("main #1") || !line.contains("main #2"), "two rows collapsed onto one line:\n{narrow_frame}");
        }
        let running_line = narrow_frame.lines().find(|l| l.contains("running")).expect("running row present");
        assert!(running_line.contains("1.0M tok"), "status cell must stay whole: {running_line}");
        assert!(!running_line.contains("opus-5-xl"), "the model cell should have been dropped:\n{narrow_frame}");

        let wide_backend = TestBackend::new(140, 36);
        let mut wide = Terminal::new(wide_backend).unwrap();
        wide.draw(|f| draw(f, &mut app)).unwrap();
        let wide_frame = frame_text(&wide);
        assert!(wide_frame.contains("opus-5-xl"), "the model cell should be back at 140 columns:\n{wide_frame}");
    }

    #[test]
    fn the_queue_strip_draws_one_row_per_line_when_two_are_queued() {
        let backend = TestBackend::new(110, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(None);
        app.submit_line("first turn".to_string());
        app.submit_line("second line".to_string());
        app.submit_line("third line".to_string());

        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let frame = frame_text(&terminal);
        assert!(frame.contains("queued 2 \u{b7} \u{2191} edits the last"), "frame:\n{frame}");
        assert!(frame.contains("second line"), "frame:\n{frame}");
        assert!(frame.contains("third line"), "frame:\n{frame}");
        assert!(!frame.contains("\u{2026} and"), "no collapse row expected under the cap:\n{frame}");
    }

    #[test]
    fn the_queue_strip_collapses_past_four_rows_and_disappears_once_empty() {
        let backend = TestBackend::new(110, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(None);
        app.submit_line("first turn".to_string());
        for i in 1..=6 {
            app.submit_line(format!("queued line {i}"));
        }

        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let frame = frame_text(&terminal);
        assert!(frame.contains("queued 6 \u{b7} \u{2191} edits the last"), "frame:\n{frame}");
        assert!(frame.contains("queued line 1"), "frame:\n{frame}");
        assert!(frame.contains("queued line 2"), "frame:\n{frame}");
        assert!(frame.contains("queued line 3"), "frame:\n{frame}");
        assert!(frame.contains("\u{2026} and 3 more"), "frame:\n{frame}");
        assert!(!frame.contains("queued line 4"), "collapsed lines must not appear individually:\n{frame}");

        app.queue.clear();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let empty_frame = frame_text(&terminal);
        assert!(!empty_frame.contains("queued"), "an empty queue draws nothing:\n{empty_frame}");
    }

    #[test]
    fn the_main_panel_shows_a_running_incarnation_ticking_and_an_ended_one_with_its_tokens() {
        let backend = TestBackend::new(140, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(None);

        app.state.apply_terminal(&TerminalMsg::Usage { input: 900, output: 100, cache_creation: 0, cache_read: 0 });
        app.state.apply_event(&Event::Reset { phase: "respawned".to_string(), check: None });
        app.state.apply_terminal(&TerminalMsg::Usage { input: 1000, output: 200, cache_creation: 0, cache_read: 300 });
        app.state.push_operator_line("hi");

        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let frame = frame_text(&terminal);
        assert!(frame.contains("ended \u{b7} 1.0k tok"), "frame:\n{frame}");
        assert!(frame.contains("running \u{b7} 0.0s \u{b7} 1.5k tok"), "frame:\n{frame}");
    }
}
