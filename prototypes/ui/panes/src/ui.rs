use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::{is_split_party, App, FocusTarget, Half, Mode, Pane, PaneKey, PanelId, PartyKey, ScrollState};
use crate::keys::{KeyMap, KeyTable, INSERT_ACTIONS, MODAL_ACTIONS, NORMAL_ACTIONS, SPLIT_ACTIONS};
use crate::scenario::{ModelSource, NoticeKind, Outcome, Role, TraceTurn};
use crate::state::{
    HandRow, Line as TLine, SessionState, GROUNDING, MODELS, NUDGE_THRESHOLD, RESERVE, SCOPE, SLASH_COMMANDS,
    STORE_ROOT, WINDOW,
};

const SPINNER: [char; 4] = ['-', '\\', '|', '/'];

pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();
    let root = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(size);
    let cols = Layout::horizontal([Constraint::Percentage(66), Constraint::Percentage(34)]).split(root[0]);

    let panel_rects = draw_right_column(f, app, cols[1]);
    app.set_layout_rects(cols[0], panel_rects);
    draw_big_view(f, app, cols[0]);
    draw_hint_bar(f, app, root[1]);

    match app.modal.clone() {
        Some(crate::app::Modal::Ask { tool, subject, selected, note }) => {
            draw_ask_modal(f, size, &tool, &subject, selected, &note)
        }
        Some(crate::app::Modal::ModelPicker { models, current, selected }) => {
            draw_model_picker(f, size, &models, &current, selected)
        }
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

fn party_title(app: &App) -> String {
    match app.current_party() {
        PartyKey::Session => "session".to_string(),
        PartyKey::Main(i) => app.state.mains.get(i).map(|m| m.label.clone()).unwrap_or_else(|| "main".to_string()),
        PartyKey::Judge(i) => {
            app.state.judges.get(i).map(|h| format!("{} {}", h.id, h.agent)).unwrap_or_else(|| "judge".to_string())
        }
        PartyKey::Hand(i) => {
            app.state.hands.get(i).map(|h| format!("{} {}", h.id, h.agent)).unwrap_or_else(|| "hand".to_string())
        }
    }
}

fn party_short(app: &App) -> String {
    match app.current_party() {
        PartyKey::Hand(i) => app.state.hands.get(i).map(|h| h.id.clone()).unwrap_or_default(),
        PartyKey::Judge(i) => app.state.judges.get(i).map(|h| h.id.clone()).unwrap_or_default(),
        _ => String::new(),
    }
}

fn single_page_lines(app: &App) -> Vec<Line<'static>> {
    match app.current_party() {
        PartyKey::Session => session_page_lines(&app.state),
        PartyKey::Main(idx) => main_page_lines(app, idx),
        PartyKey::Judge(_) | PartyKey::Hand(_) => Vec::new(),
    }
}

fn main_page_lines(app: &App, idx: usize) -> Vec<Line<'static>> {
    let Some(main) = app.state.mains.get(idx) else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    if let Some(reason) = &main.ended {
        lines.push(Line::from(Span::styled(
            format!("ended: {reason}"),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
    }
    lines.extend(transcript_lines(&main.transcript));

    let is_live = idx == app.state.current_main && main.ended.is_none();
    if is_live {
        if let Some(buf) = &app.state.streaming_buffer {
            lines.extend(render_markdown(buf));
        } else if app.is_streaming() {
            let elapsed = app.waiting_since.map(|s| s.elapsed().as_secs_f32()).unwrap_or(0.0);
            let frame = SPINNER[(elapsed * 4.0) as usize % SPINNER.len()];
            let verb = if app.state.pending_tool.is_some() { "using tool" } else { "thinking" };
            lines.push(Line::from(Span::styled(
                format!("{frame} {verb} {elapsed:.1}s"),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }
    lines
}

fn brief_pane_lines(row: &HandRow, session_started: std::time::Instant) -> Vec<Line<'static>> {
    let role = match row.role {
        Role::Hand => "hand",
        Role::Judge => "judge",
    };
    let spawn_t = row.spawned_at.saturating_duration_since(session_started).as_secs_f32();
    let mut lines = vec![
        Line::from(Span::styled(
            format!(
                "{} \u{b7} {} \u{b7} {role} \u{b7} {} \u{b7} {} \u{b7} {} \u{b7} t+{spawn_t:.1}s",
                row.id, row.agent, row.body, row.container, row.model
            ),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    lines.extend(render_markdown(&row.brief));
    lines
}

fn render_trace_turns(turns: &[TraceTurn]) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for turn in turns {
        match turn {
            TraceTurn::Text(text) => out.extend(render_markdown(text)),
            TraceTurn::ToolCall { tool, args } => {
                out.push(Line::from(Span::styled(format!("\u{b7} {tool} {args}"), Style::default().fg(Color::DarkGray))));
            }
            TraceTurn::ToolResult(text) => {
                for l in text.lines().take(3) {
                    out.push(Line::from(Span::styled(format!("  {l}"), Style::default().fg(Color::DarkGray))));
                }
            }
        }
    }
    out
}

fn verdict_block_lines(row: &HandRow) -> Vec<Line<'static>> {
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
        lines.extend(render_markdown(text));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(format!("trace     {}", row.trace.clone().unwrap_or_default())));
    lines.push(Line::from(format!("bytes     {}", row.bytes.unwrap_or(0))));
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

fn turns_pane_lines(row: &HandRow) -> Vec<Line<'static>> {
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
                lines.extend(render_markdown(question));
            }
            lines
        }
        Some(Outcome::Killed) => verdict_block_lines(row),
        Some(Outcome::Verdict) => {
            let mut lines = render_trace_turns(&row.turns);
            lines.push(Line::from(""));
            lines.extend(verdict_block_lines(row));
            lines
        }
    }
}

fn session_page_lines(state: &SessionState) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(Span::styled("session", Style::default().add_modifier(Modifier::BOLD))),
        Line::from(format!("model       {}", state.model)),
        Line::from(format!("scope       {SCOPE}")),
        Line::from(format!("store root  {STORE_ROOT}")),
        Line::from(format!("grounding   {GROUNDING}")),
        Line::from(""),
        occupancy_bar(state.occupancy, state.gauge_alert(), 60),
        Line::from(format!("occupancy   {} / {WINDOW}", state.occupancy)),
        Line::from(format!("reserve     {RESERVE}")),
        Line::from(format!("checkpoint  {} ({:+})", state.checkpoint_number, state.checkpoint_delta)),
        Line::from(format!("turns       {}", state.turn_count)),
        Line::from(format!("hands in flight  {}", state.hands_in_flight())),
        Line::from(""),
        Line::from(format!(
            "gate allow {} \u{b7} deny {} \u{b7} ask {}",
            state.gate.allow, state.gate.deny, state.gate.ask
        )),
    ];
    if state.gate.recent.is_empty() {
        lines.push(Line::from("recent: none yet"));
    } else {
        lines.push(Line::from("recent:"));
        for entry in state.gate.recent.iter().rev() {
            let verdict = if entry.allowed { "allow" } else { "deny" };
            lines.push(Line::from(format!("  \u{b7} {verdict} \u{b7} {}", entry.rule)));
        }
    }
    if state.gate.durable_rules.is_empty() {
        lines.push(Line::from("durable rules: none"));
    } else {
        lines.push(Line::from("durable rules:"));
        for r in &state.gate.durable_rules {
            lines.push(Line::from(format!("  \u{b7} {r}")));
        }
    }
    match &state.gate.pending {
        Some((tool, subject)) => lines.push(Line::from(format!("pending ask: {tool} {subject}"))),
        None => lines.push(Line::from("pending ask: none")),
    }
    if let Some(u) = &state.last_usage {
        let source = match u.source {
            ModelSource::Sdk => "sdk",
            ModelSource::Assumed => "assumed",
        };
        lines.push(Line::from(format!(
            "last turn   {}/{}/{} tok \u{b7} {} ({source}) \u{b7} {:.1}s",
            u.input,
            u.output,
            u.cache_read,
            short_model(&u.model),
            u.elapsed_ms as f32 / 1000.0
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("models"));
    for m in MODELS {
        let marker = if m == state.model { "\u{25b8} " } else { "  " };
        let tag = if m == state.model { " (current)" } else { "" };
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
    let source_len = lines.len();
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

fn pane_border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn draw_big_view(f: &mut Frame, app: &mut App, area: Rect) {
    let input_lines = 1 + app.input.matches('\n').count();
    let input_height = (input_lines as u16 + 2).clamp(3, 7);
    let parts = Layout::vertical([Constraint::Min(3), Constraint::Length(input_height)]).split(area);

    if is_split_party(app.current_party()) {
        draw_split_view(f, app, parts[0]);
    } else {
        let title = party_title(app);
        let key = PaneKey(app.current_party(), Pane::Single);
        let border_style =
            if matches!(app.focus, FocusTarget::BigView(_)) { Style::default().fg(Color::Cyan) } else { Style::default() };
        let lines = single_page_lines(app);
        draw_scrollable_pane(f, app, parts[0], key, lines, &title, border_style);
    }

    draw_input(f, app, parts[1]);

    if app.mode == Mode::Insert && app.input == "/" {
        draw_slash_popup(f, area);
    }
}

fn draw_split_view(f: &mut Frame, app: &mut App, area: Rect) {
    let title = party_title(app);
    let outer = Block::default().borders(Borders::ALL).title(title);
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let brief_h = (inner.height / 3).clamp(3, inner.height.saturating_sub(3).max(3));
    let halves = Layout::vertical([Constraint::Length(brief_h), Constraint::Min(3)]).split(inner);

    let party = app.current_party();
    let row = match party {
        PartyKey::Hand(i) => app.state.hands.get(i),
        PartyKey::Judge(i) => app.state.judges.get(i),
        _ => None,
    };
    let session_started = app.state.session_started;
    let big_view_focused = matches!(app.focus, FocusTarget::BigView(_));
    let brief_focused = big_view_focused && app.half_focus == Half::Brief;
    let turns_focused = big_view_focused && app.half_focus == Half::Turns;

    let brief_lines = row.map(|r| brief_pane_lines(r, session_started)).unwrap_or_default();
    let turns_lines = row.map(turns_pane_lines).unwrap_or_default();

    draw_scrollable_pane(f, app, halves[0], PaneKey(party, Pane::Brief), brief_lines, "brief", pane_border_style(brief_focused));
    draw_scrollable_pane(f, app, halves[1], PaneKey(party, Pane::Turns), turns_lines, "turns", pane_border_style(turns_focused));
}

fn draw_input(f: &mut Frame, app: &App, area: Rect) {
    let insert = app.mode == Mode::Insert;
    let border_style =
        if insert { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) };
    let text_style = if insert { Style::default() } else { Style::default().fg(Color::DarkGray) };
    let block = Block::default().borders(Borders::ALL).title("input").border_style(border_style);
    let input_text = Line::from(Span::styled(format!("> {}", app.input), text_style));
    let widget = Paragraph::new(input_text).block(block).wrap(Wrap { trim: false });
    f.render_widget(widget, area);
    if insert && app.modal.is_none() && !app.help_visible {
        let cursor_x = area.x + 1 + 2 + UnicodeWidthStr::width(app.input.as_str()) as u16;
        let cursor_x = cursor_x.min(area.x + area.width.saturating_sub(2));
        f.set_cursor_position((cursor_x, area.y + 1));
    }
}

fn transcript_lines(transcript: &[TLine]) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for entry in transcript {
        match entry {
            TLine::Operator(text) => {
                out.push(Line::from(Span::styled(format!("> {text}"), Style::default().add_modifier(Modifier::BOLD))));
            }
            TLine::Main(text) => out.extend(render_markdown(text)),
            TLine::ToolSummary(text) => out.push(Line::from(Span::styled(text.clone(), Style::default().fg(Color::DarkGray)))),
            TLine::DispatchSummary(text) => out.push(Line::from(Span::styled(text.clone(), Style::default().fg(Color::DarkGray)))),
            TLine::Delivery { id, text } => {
                out.push(Line::from(Span::styled(format!("{id} delivered: {text}"), Style::default().fg(Color::Green))));
            }
            TLine::ArxPrompt(text) => {
                out.push(Line::from(Span::styled(
                    format!("[arx] {text}"),
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                )));
            }
            TLine::Notice { kind, text } => {
                let style = notice_style(*kind);
                out.push(Line::from(Span::styled(format!("[arx] {text}"), style)));
            }
            TLine::AbortedPartial(text) => {
                out.extend(render_markdown(text));
                out.push(Line::from(Span::styled("[arx] aborted", Style::default().fg(Color::Yellow))));
            }
            TLine::ResetConfirmBlock(text) => {
                out.push(Line::from(Span::styled(
                    "\u{250c}\u{2500} state object \u{2500}\u{2500}\u{2500}",
                    Style::default().fg(Color::DarkGray),
                )));
                for l in text.split('\n') {
                    out.push(Line::from(Span::styled(format!("\u{2502} {l}"), Style::default().fg(Color::Gray))));
                }
                out.push(Line::from(Span::styled(
                    "\u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }
    }
    out
}

fn notice_style(kind: NoticeKind) -> Style {
    match kind {
        NoticeKind::Info => Style::default().fg(Color::DarkGray),
        NoticeKind::Nudge => Style::default().fg(Color::Yellow),
        NoticeKind::ResetArmed => Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        NoticeKind::Error => Style::default().fg(Color::Red),
    }
}

fn render_markdown(text: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut in_code = false;
    for raw in text.split('\n') {
        if raw.trim() == "```" {
            in_code = !in_code;
            lines.push(Line::from(Span::styled("\u{2508}".repeat(30), Style::default().fg(Color::DarkGray))));
            continue;
        }
        if in_code {
            lines.push(Line::from(Span::styled(format!("\u{2502} {raw}"), Style::default().fg(Color::Gray))));
            continue;
        }
        if raw.trim_start().starts_with("- ") {
            let indent = raw.len() - raw.trim_start().len();
            let rest = raw.trim_start().trim_start_matches("- ");
            let mut spans = vec![Span::raw(" ".repeat(indent) + "\u{2022} ")];
            spans.extend(inline_spans(rest));
            lines.push(Line::from(spans));
            continue;
        }
        lines.push(Line::from(inline_spans(raw)));
    }
    lines
}

fn inline_spans(raw: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut chars = raw.chars().peekable();
    let mut buf = String::new();
    while let Some(c) = chars.next() {
        if c == '*' && chars.peek() == Some(&'*') {
            chars.next();
            if !buf.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut buf)));
            }
            let mut bold = String::new();
            while let Some(&nc) = chars.peek() {
                if nc == '*' {
                    chars.next();
                    if chars.peek() == Some(&'*') {
                        chars.next();
                    }
                    break;
                }
                bold.push(nc);
                chars.next();
            }
            spans.push(Span::styled(bold, Style::default().add_modifier(Modifier::BOLD)));
        } else if c == '`' {
            if !buf.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut buf)));
            }
            let mut code = String::new();
            while let Some(&nc) = chars.peek() {
                if nc == '`' {
                    chars.next();
                    break;
                }
                code.push(nc);
                chars.next();
            }
            spans.push(Span::styled(code, Style::default().fg(Color::Cyan)));
        } else {
            buf.push(c);
        }
    }
    if !buf.is_empty() {
        spans.push(Span::raw(buf));
    }
    spans
}

fn occupancy_bar(occupancy: u32, alert: crate::state::GaugeAlert, width: usize) -> Line<'static> {
    let width = width.max(10);
    let occ_idx = (((occupancy as f64 / WINDOW as f64) * width as f64).round() as usize).min(width);
    let ceiling_idx = (((WINDOW - RESERVE) as f64 / WINDOW as f64) * width as f64).round() as usize;
    let nudge_idx = ((NUDGE_THRESHOLD as f64 / WINDOW as f64) * width as f64).round() as usize;
    let mut chars: Vec<char> = (0..width).map(|i| if i < occ_idx { '#' } else { '.' }).collect();
    if nudge_idx < width {
        chars[nudge_idx] = ':';
    }
    if ceiling_idx < width {
        chars[ceiling_idx] = '|';
    }
    let color = match alert {
        crate::state::GaugeAlert::Normal => Color::Green,
        crate::state::GaugeAlert::Nudge => Color::Yellow,
        crate::state::GaugeAlert::ResetArmed => Color::LightRed,
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

fn short_model(model: &str) -> &str {
    model.strip_prefix("claude-").unwrap_or(model)
}

fn panel_block(id: PanelId, active: bool) -> Block<'static> {
    let title = format!("{} {}", id.number(), id.name());
    let style = if active { Style::default().fg(Color::Cyan) } else { Style::default() };
    Block::default().borders(Borders::ALL).title(title).border_style(style)
}

fn draw_right_column(f: &mut Frame, app: &App, area: Rect) -> [Rect; 4] {
    let main_h = (app.state.mains.len() as u16 + 2).max(3);
    let judges_h = (app.state.judges.len() as u16 + 2).max(3);
    let parts = Layout::vertical([
        Constraint::Length(9),
        Constraint::Length(main_h),
        Constraint::Length(judges_h),
        Constraint::Min(3),
    ])
    .split(area);

    draw_session_panel(f, app, parts[0]);
    draw_main_panel(f, app, parts[1]);
    draw_judges_panel(f, app, parts[2]);
    draw_hands_panel(f, app, parts[3]);
    [parts[0], parts[1], parts[2], parts[3]]
}

fn draw_session_panel(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel() == PanelId::Session;
    let s = &app.state;
    let mut lines = vec![
        Line::from(format!("model  {}", s.model)),
        occupancy_bar(s.occupancy, s.gauge_alert(), (area.width as usize).saturating_sub(4).max(10)),
        Line::from(format!("{} / {WINDOW}", s.occupancy)),
        Line::from(format!("reserve {RESERVE}  cp {}", s.checkpoint_number)),
        Line::from(format!("gate allow {} ask {}", s.gate.allow, s.gate.ask)),
        Line::from(format!(
            "rules +{}  \u{b7} pending {}",
            s.gate.durable_rules.len(),
            if s.gate.pending.is_some() { 1 } else { 0 }
        )),
    ];
    if let Some(u) = &s.last_usage {
        lines.push(Line::from(format!("last {}/{}/{} \u{b7} {:.1}s", u.input, u.output, u.cache_read, u.elapsed_ms as f32 / 1000.0)));
    }
    f.render_widget(Paragraph::new(lines).block(panel_block(PanelId::Session, active)).wrap(Wrap { trim: false }), area);
}

fn draw_main_panel(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel() == PanelId::Main;
    let mut lines = Vec::new();
    for (i, m) in app.state.mains.iter().enumerate().rev() {
        let status = if m.ended.is_some() { "ended" } else { "live" };
        let text = format!("{}  {}  {}", m.label, short_model(&m.model), status);
        let selected = i == app.main_selected;
        let style = if selected { Style::default().fg(Color::Black).bg(Color::Cyan) } else { Style::default() };
        lines.push(Line::from(Span::styled(text, style)));
    }
    f.render_widget(Paragraph::new(lines).block(panel_block(PanelId::Main, active)).wrap(Wrap { trim: false }), area);
}

fn draw_judges_panel(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel() == PanelId::Judges;
    draw_party_list(f, PanelId::Judges, active, &app.state.judges, app.judge_selected, area);
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
        let text = if drop_agent {
            format!("{}  {}", cell(&row.id, 7), row.status_cell())
        } else {
            format!("{}  {}  {}", cell(&row.id, 7), cell(&row.agent, 11), row.status_cell())
        };
        let style = if i == selected { Style::default().fg(Color::Black).bg(Color::Cyan) } else { Style::default() };
        lines.push(Line::from(Span::styled(text, style)));
    }
    f.render_widget(Paragraph::new(lines).block(panel_block(id, active)).wrap(Wrap { trim: false }), area);
}

fn primary<'a>(table: &'a KeyTable, action: &str) -> &'a str {
    table.primary(action).unwrap_or("?")
}

fn draw_hint_bar(f: &mut Frame, app: &App, area: Rect) {
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
    if app.is_streaming() && app.modal.is_none() {
        hints.push(format!("{} abort", primary(&app.keys.normal, "abort")));
    }
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
    let rect = Rect {
        x: area.x + 2,
        y: area.y.saturating_add(area.height.saturating_sub(9)),
        width: 40.min(area.width.saturating_sub(4)),
        height: 6,
    };
    let lines: Vec<Line> = SLASH_COMMANDS.iter().map(|c| Line::from(*c)).collect();
    let block = Block::default().borders(Borders::ALL).title("commands");
    f.render_widget(Clear, rect);
    f.render_widget(Paragraph::new(lines).block(block), rect);
}

fn draw_ask_modal(f: &mut Frame, area: Rect, tool: &str, subject: &str, selected: usize, note: &str) {
    let rect = centered(area, 70, 13);
    f.render_widget(Clear, rect);
    let mut lines = vec![
        Line::from(Span::styled(format!("gate ask: {tool}"), Style::default().add_modifier(Modifier::BOLD))),
        Line::from(subject.to_string()),
        Line::from(""),
    ];
    for (i, (key, label)) in
        [('a', "allow once"), ('A', "allow always"), ('d', "deny once"), ('D', "deny always")].iter().enumerate()
    {
        let marker = if i == selected { "\u{25b8} " } else { "  " };
        let style = if i == selected { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default() };
        lines.push(Line::from(Span::styled(format!("{marker}{key}  {label}"), style)));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(format!("note: {note}")));
    lines.push(Line::from(Span::styled(
        "a A d D or arrows to choose \u{b7} type a note \u{b7} Enter to submit",
        Style::default().fg(Color::DarkGray),
    )));
    let block = Block::default().borders(Borders::ALL).title("ask").border_style(Style::default().fg(Color::Cyan));
    f.render_widget(Paragraph::new(lines).block(block).wrap(Wrap { trim: false }), rect);
}

fn draw_model_picker(f: &mut Frame, area: Rect, models: &[String], current: &str, selected: usize) {
    let rect = centered(area, 40, (models.len() as u16) + 4);
    f.render_widget(Clear, rect);
    let mut lines = Vec::new();
    for (i, m) in models.iter().enumerate() {
        let marker = if i == selected { "\u{25b8} " } else { "  " };
        let current_tag = if m == current { " (current)" } else { "" };
        let style = if i == selected { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default() };
        lines.push(Line::from(Span::styled(format!("{marker}{}. {m}{current_tag}", i + 1), style)));
    }
    lines.push(Line::from(Span::styled("j/k or arrows, digits, Enter, Esc", Style::default().fg(Color::DarkGray))));
    let block = Block::default().borders(Borders::ALL).title("/model").border_style(Style::default().fg(Color::Cyan));
    f.render_widget(Paragraph::new(lines).block(block), rect);
}

fn draw_help_overlay(f: &mut Frame, area: Rect, keys: &KeyMap) {
    let rect = centered(area, 74, area.height.saturating_sub(2).min(30));
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
