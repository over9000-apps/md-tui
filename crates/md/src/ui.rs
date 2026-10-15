//! Draw the reader and run the terminal loop.
//!
//! Render is a pure function of `App` plus the frame size. After ratatui
//! flushes a frame, OSC 8 hyperlinks are painted on top of link spans so
//! terminals that understand them can open those URLs too.

use std::io::{self, stdout, Write};
use std::time::Duration;

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::app::{App, HeadingHit, HitBox, KeyInput};
use crate::layout::{DocLine, LinkInfo, Rendered};
use crate::theme::{Face, Paint, StyleId, Theme};

pub fn run(mut app: App) -> Result<(), String> {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        previous(info);
    }));

    enable_raw_mode().map_err(|err| err.to_string())?;
    let mut out = stdout();
    execute!(
        out,
        EnterAlternateScreen,
        EnableMouseCapture,
        crossterm::cursor::Hide
    )
    .map_err(|err| {
        restore_terminal();
        err.to_string()
    })?;
    // Any-motion tracking so hover can light up links. 1002 (button-drag) is
    // what crossterm enables; 1003 reports moves with no button held.
    let _ = write!(out, "\x1b[?1003h");
    let _ = out.flush();

    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend).map_err(|err| {
        restore_terminal();
        err.to_string()
    })?;

    let result = event_loop(&mut terminal, &mut app);
    restore_terminal();
    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<(), String> {
    let mut dirty = true;
    loop {
        if dirty {
            let size = terminal.size().map_err(|err| err.to_string())?;
            let metrics = metrics(size.width, size.height, app);
            app.last_view_rows = metrics.view_rows;
            app.relayout(metrics.content_width);
            app.clamp_scroll();
            terminal
                .draw(|frame| draw(frame, app))
                .map_err(|err| err.to_string())?;
            paint_osc8(&app.hits).map_err(|err| err.to_string())?;
            dirty = false;
        }
        if app.quit {
            break;
        }
        if !event::poll(Duration::from_millis(150)).map_err(|err| err.to_string())? {
            continue;
        }
        match event::read().map_err(|err| err.to_string())? {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                app.handle_key(map_key(key.code, key.modifiers));
                dirty = true;
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => {
                    app.scroll_by(-3);
                    dirty = true;
                }
                MouseEventKind::ScrollDown => {
                    app.scroll_by(3);
                    dirty = true;
                }
                MouseEventKind::ScrollLeft => {
                    app.hscroll_by(-4);
                    dirty = true;
                }
                MouseEventKind::ScrollRight => {
                    app.hscroll_by(4);
                    dirty = true;
                }
                MouseEventKind::Moved => {
                    if app.hover_at(mouse.column, mouse.row) {
                        dirty = true;
                    }
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    app.click_at(mouse.column, mouse.row);
                    dirty = true;
                }
                _ => {}
            },
            Event::Resize(_, _) => dirty = true,
            _ => {}
        }
    }
    Ok(())
}

fn map_key(code: KeyCode, mods: KeyModifiers) -> KeyInput {
    if mods.contains(KeyModifiers::CONTROL) {
        if let KeyCode::Char(c) = code {
            return KeyInput::Ctrl(c.to_ascii_lowercase());
        }
    }
    if mods.contains(KeyModifiers::ALT) && matches!(code, KeyCode::Left) {
        return KeyInput::Ctrl('b');
    }
    match code {
        KeyCode::Char(c) => KeyInput::Char(c),
        KeyCode::Backspace => KeyInput::Backspace,
        KeyCode::Enter => KeyInput::Enter,
        KeyCode::Esc => KeyInput::Esc,
        KeyCode::Tab => KeyInput::Tab,
        KeyCode::BackTab => KeyInput::BackTab,
        KeyCode::Up => KeyInput::Up,
        KeyCode::Down => KeyInput::Down,
        KeyCode::Left => KeyInput::Left,
        KeyCode::Right => KeyInput::Right,
        KeyCode::Home => KeyInput::Home,
        KeyCode::End => KeyInput::End,
        KeyCode::PageUp => KeyInput::PageUp,
        KeyCode::PageDown => KeyInput::PageDown,
        _ => KeyInput::Char('\0'),
    }
}

pub fn restore_terminal() {
    let mut out = stdout();
    let _ = write!(out, "\x1b[?1003l\x1b[?1002l\x1b[?1000l");
    let _ = execute!(
        out,
        DisableMouseCapture,
        LeaveAlternateScreen,
        crossterm::cursor::Show
    );
    let _ = disable_raw_mode();
    let _ = out.flush();
}

struct Metrics {
    content_width: usize,
    view_rows: usize,
}

fn metrics(width: u16, height: u16, app: &App) -> Metrics {
    let search = if app.searching || !app.query.is_empty() {
        1
    } else {
        0
    };
    let pad_top = top_pad(height, search);
    let view_rows = (height as usize)
        .saturating_sub(1 + search + pad_top)
        .max(1);
    let outline = if app.outline { outline_width(width) } else { 0 };
    // Two columns of inset plus the scrollbar, matching draw_doc.
    let content_width = width.saturating_sub(outline).saturating_sub(3).max(1) as usize;
    Metrics {
        content_width,
        view_rows,
    }
}

fn top_pad(height: u16, search: usize) -> usize {
    let body = height.saturating_sub(1 + search as u16);
    if body > 8 {
        1
    } else {
        0
    }
}

fn outline_width(total: u16) -> u16 {
    if total < 36 {
        total / 3
    } else {
        28.min(total / 3).max(18)
    }
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let theme = app.theme();
    let buf = frame.buffer_mut();
    let base = if theme.fill_bg {
        style_from_face(Face {
            fg: theme.text,
            bg: Some(theme.bg),
            bold: false,
            italic: false,
            underline: false,
            strike: false,
        })
    } else {
        Style::default()
    };
    buf.set_style(area, base);

    let search_h: u16 = if app.searching || !app.query.is_empty() {
        1
    } else {
        0
    };
    let status_y = area.height.saturating_sub(1);
    let search_y = status_y.saturating_sub(search_h);
    let body_h = area.height.saturating_sub(1 + search_h);

    let outline_w = if app.outline {
        outline_width(area.width).min(area.width.saturating_sub(10))
    } else {
        0
    };
    app.heading_hits.clear();
    app.hits.clear();

    if outline_w > 0 {
        let rect = Rect {
            x: area.x,
            y: area.y,
            width: outline_w,
            height: body_h,
        };
        draw_outline(buf, rect, app, &theme);
    }

    let pad_top = top_pad(area.height, search_h as usize) as u16;
    let doc = Rect {
        x: area.x + outline_w,
        y: area.y + pad_top.min(body_h.saturating_sub(1)),
        width: area.width.saturating_sub(outline_w),
        height: body_h.saturating_sub(pad_top),
    };
    if doc.width > 0 && doc.height > 0 {
        draw_doc(buf, doc, app, &theme);
    }

    if search_h == 1 {
        draw_search(
            buf,
            Rect {
                x: area.x,
                y: area.y + search_y,
                width: area.width,
                height: 1,
            },
            app,
            &theme,
        );
    }
    draw_status(
        buf,
        Rect {
            x: area.x,
            y: area.y + status_y,
            width: area.width,
            height: 1,
        },
        app,
        &theme,
    );

    if app.help {
        draw_help(frame, app, &theme);
        app.hits.clear();
        app.heading_hits.clear();
    } else if app.picker {
        draw_picker(frame, app, &theme);
        app.hits.clear();
    }
}

fn draw_doc(buf: &mut ratatui::buffer::Buffer, area: Rect, app: &mut App, theme: &Theme) {
    let scrollbar_w = if area.width > 4 { 1 } else { 0 };
    let text_w = area.width.saturating_sub(scrollbar_w + 2);
    let origin_x = area.x.saturating_add(2);
    if app.rendered.lines.is_empty() {
        let msg = Span::styled(
            "This file is empty.",
            face_style(theme, StyleId::Muted, theme.fill_bg),
        );
        buf.set_line(origin_x, area.y, &Line::from(msg), text_w);
        return;
    }
    for row in 0..area.height {
        let line_idx = app.scroll + row as usize;
        let y = area.y + row;
        if let Some(line) = app.rendered.lines.get(line_idx) {
            let matched = app.match_span(line_idx);
            let ctx = PaintCtx {
                theme,
                hscroll: app.hscroll,
                hover: app.hover_link,
                focus: app.focus_link,
                matches: &matched.spans,
                current: matched.current,
                links: &app.rendered.links,
            };
            let hits = paint_line(buf, origin_x, y, text_w, line, &ctx);
            app.hits.extend(hits);
        } else if theme.fill_bg {
            clear_row(buf, origin_x, y, text_w, theme.bg, theme.text);
        }
    }
    if scrollbar_w == 1 {
        draw_scrollbar(
            buf,
            area.x + area.width - 1,
            area.y,
            area.height,
            app.scroll,
            app.rendered.lines.len(),
            theme,
        );
    }
}

struct PaintCtx<'a> {
    theme: &'a Theme,
    hscroll: usize,
    hover: Option<u32>,
    focus: Option<u32>,
    matches: &'a [(usize, usize)],
    current: Option<(usize, usize)>,
    links: &'a [LinkInfo],
}

fn paint_line(
    buf: &mut ratatui::buffer::Buffer,
    x0: u16,
    y: u16,
    width: u16,
    line: &DocLine,
    ctx: &PaintCtx<'_>,
) -> Vec<HitBox> {
    if width == 0 {
        return Vec::new();
    }
    let mut pieces: Vec<Piece> = Vec::new();
    let mut col = 0usize;
    for seg in &line.spans {
        let url = seg
            .link
            .and_then(|id| ctx.links.iter().find(|link| link.id == id))
            .map(|link| link.url.clone())
            .unwrap_or_default();
        for grapheme in seg.text.graphemes(true) {
            let w = UnicodeWidthStr::width(grapheme);
            if w == 0 {
                if col >= ctx.hscroll {
                    if let Some(piece) = pieces.last_mut() {
                        piece.text.push_str(grapheme);
                    }
                }
                continue;
            }
            let start = col;
            col += w;
            if col <= ctx.hscroll || start < ctx.hscroll {
                continue;
            }
            let vis = start - ctx.hscroll;
            if vis >= width as usize || vis + w > width as usize {
                continue;
            }
            let kind = match_kind(start, w, ctx.matches, ctx.current);
            let (style, sgr) = visual_style(
                ctx.theme, seg.style, line.code, seg.link, ctx.hover, ctx.focus, kind,
            );
            if let Some(piece) = pieces.last_mut() {
                if piece.style == style && piece.link == seg.link {
                    piece.text.push_str(grapheme);
                    continue;
                }
            }
            pieces.push(Piece {
                text: grapheme.to_string(),
                style,
                sgr,
                link: seg.link,
                url: url.clone(),
            });
        }
    }

    let mut spans = Vec::new();
    let mut hits: Vec<HitBox> = Vec::new();
    let mut used = 0usize;
    let mut screen_x = x0;
    for piece in pieces {
        let w = UnicodeWidthStr::width(piece.text.as_str());
        if let Some(id) = piece.link {
            if let Some(hit) = hits.last_mut() {
                if hit.link_id == id && hit.x as usize + hit.width == screen_x as usize {
                    hit.text.push_str(&piece.text);
                    hit.width += w;
                } else {
                    hits.push(hit_from(&piece, screen_x, y, w, id));
                }
            } else {
                hits.push(hit_from(&piece, screen_x, y, w, id));
            }
        }
        spans.push(Span::styled(piece.text, piece.style));
        used += w;
        screen_x = screen_x.saturating_add(w as u16);
    }
    if used < width as usize {
        let fill = fill_style(ctx.theme, line.code);
        spans.push(Span::styled(" ".repeat(width as usize - used), fill));
    }
    buf.set_line(x0, y, &Line::from(spans), width);
    hits
}

struct Piece {
    text: String,
    style: Style,
    sgr: String,
    link: Option<u32>,
    url: String,
}

fn hit_from(piece: &Piece, x: u16, y: u16, width: usize, id: u32) -> HitBox {
    HitBox {
        x,
        y,
        width,
        text: piece.text.clone(),
        url: piece.url.clone(),
        link_id: id,
        sgr: piece.sgr.clone(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MatchKind {
    None,
    Hit,
    Current,
}

fn match_kind(
    col: usize,
    w: usize,
    matches: &[(usize, usize)],
    current: Option<(usize, usize)>,
) -> MatchKind {
    let end = col + w;
    if let Some((start, stop)) = current {
        if col < stop && end > start {
            return MatchKind::Current;
        }
    }
    if matches
        .iter()
        .any(|(start, stop)| col < *stop && end > *start)
    {
        MatchKind::Hit
    } else {
        MatchKind::None
    }
}

fn visual_style(
    theme: &Theme,
    id: StyleId,
    code_line: bool,
    link: Option<u32>,
    hover: Option<u32>,
    focus: Option<u32>,
    kind: MatchKind,
) -> (Style, String) {
    let mut face = theme.face(id);
    if code_line && face.bg.is_none() {
        face.bg = Some(theme.code_bg);
    } else if theme.fill_bg && face.bg.is_none() {
        face.bg = Some(theme.bg);
    }
    if let Some(id) = link {
        face.underline = true;
        if Some(id) == focus {
            face.bg = Some(theme.focus_bg);
        } else if Some(id) == hover {
            face.bg = Some(theme.link_hover_bg);
        }
    }
    match kind {
        MatchKind::Current => {
            face.bg = Some(theme.search_current_bg);
            face.fg = theme.search_fg;
            face.bold = true;
        }
        MatchKind::Hit => {
            face.bg = Some(theme.search_bg);
            face.fg = theme.search_fg;
        }
        MatchKind::None => {}
    }
    (face_to_style(face), face_to_sgr(&face))
}

fn fill_style(theme: &Theme, code: bool) -> Style {
    let bg = if code {
        theme.code_bg
    } else if theme.fill_bg {
        theme.bg
    } else {
        Paint::Reset
    };
    Style::default().bg(paint_color(bg))
}

fn clear_row(buf: &mut ratatui::buffer::Buffer, x: u16, y: u16, width: u16, bg: Paint, fg: Paint) {
    let style = Style::default().bg(paint_color(bg)).fg(paint_color(fg));
    buf.set_line(
        x,
        y,
        &Line::from(Span::styled(" ".repeat(width as usize), style)),
        width,
    );
}

fn draw_scrollbar(
    buf: &mut ratatui::buffer::Buffer,
    x: u16,
    y: u16,
    height: u16,
    scroll: usize,
    total: usize,
    theme: &Theme,
) {
    let style = face_style(theme, StyleId::Muted, true);
    let thumb_style = Style::default().fg(paint_color(theme.status_accent));
    if height == 0 {
        return;
    }
    let track = height as usize;
    if total <= track {
        for i in 0..height {
            buf.set_span(x, y + i, &Span::styled("│", style), 1);
        }
        return;
    }
    let thumb = ((track * track) / total).clamp(1, track);
    let max_scroll = total - track;
    let pos = if max_scroll == 0 {
        0
    } else {
        scroll.min(max_scroll) * (track - thumb) / max_scroll
    };
    for i in 0..track {
        let on = i >= pos && i < pos + thumb;
        let span = if on {
            Span::styled("█", thumb_style)
        } else {
            Span::styled("│", style)
        };
        buf.set_span(x, y + i as u16, &span, 1);
    }
}

fn draw_outline(buf: &mut ratatui::buffer::Buffer, area: Rect, app: &mut App, theme: &Theme) {
    let bg = if theme.fill_bg {
        theme.outline_bg
    } else {
        Paint::Reset
    };
    let base = Style::default()
        .bg(paint_color(bg))
        .fg(paint_color(theme.text));
    buf.set_style(area, base);
    let title = Span::styled(" OUTLINE", face_style(theme, StyleId::H2, true));
    buf.set_line(area.x, area.y, &Line::from(title), area.width);
    if area.height <= 1 {
        return;
    }
    let selected = app.outline_index();
    let window = (area.height - 1) as usize;
    let start = if app.rendered.headings.len() <= window {
        0
    } else {
        selected
            .saturating_sub(window / 2)
            .min(app.rendered.headings.len() - window)
    };
    for row in 1..area.height {
        let idx = start + (row as usize - 1);
        let y = area.y + row;
        let Some(heading) = app.rendered.headings.get(idx) else {
            break;
        };
        let indent = " ".repeat((heading.level.saturating_sub(1) as usize).min(4) + 1);
        let marker = if idx == selected { "▸ " } else { "  " };
        let label = format!("{indent}{marker}{}", heading.text);
        let shown = truncate(&label, area.width as usize);
        let style = if idx == selected {
            Style::default()
                .bg(paint_color(theme.outline_sel_bg))
                .fg(paint_color(theme.h1))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .bg(paint_color(bg))
                .fg(paint_color(match heading.level {
                    1 => theme.h1,
                    2 => theme.h2,
                    3 => theme.h3,
                    4 => theme.h4,
                    5 => theme.h5,
                    _ => theme.muted,
                }))
        };
        let pad = area.width as usize - UnicodeWidthStr::width(shown.as_str());
        let text = format!("{shown}{}", " ".repeat(pad));
        buf.set_line(
            area.x,
            y,
            &Line::from(Span::styled(text, style)),
            area.width,
        );
        app.heading_hits.push(HeadingHit {
            x: area.x,
            y,
            width: area.width,
            index: idx,
        });
    }
}

fn draw_search(buf: &mut ratatui::buffer::Buffer, area: Rect, app: &App, theme: &Theme) {
    let style = Style::default()
        .bg(paint_color(theme.search_bg))
        .fg(paint_color(theme.search_fg));
    let label = truncate(&app.search_label(), area.width as usize);
    let pad = " ".repeat(area.width as usize - UnicodeWidthStr::width(label.as_str()));
    buf.set_line(
        area.x,
        area.y,
        &Line::from(Span::styled(format!("{label}{pad}"), style)),
        area.width,
    );
}

fn pick_hint(left_w: usize, width: usize, hints: &[String]) -> String {
    for hint in hints {
        let hint_w = UnicodeWidthStr::width(hint.as_str());
        if left_w + 2 + hint_w <= width {
            return hint.clone();
        }
    }
    let min_left = (width / 2).min(left_w);
    for hint in hints.iter().rev() {
        let hint_w = UnicodeWidthStr::width(hint.as_str());
        if min_left + 2 + hint_w <= width {
            return hint.clone();
        }
    }
    String::new()
}

fn draw_status(buf: &mut ratatui::buffer::Buffer, area: Rect, app: &App, theme: &Theme) {
    let style = Style::default()
        .bg(paint_color(theme.status_bg))
        .fg(paint_color(theme.status_fg));
    let accent = Style::default()
        .bg(paint_color(theme.status_bg))
        .fg(paint_color(theme.status_accent));
    let theme_label = theme.name.label();
    let hints: Vec<String> = if app.pending_g {
        vec![format!("g…  {theme_label}")]
    } else {
        vec![
            format!(
                "j/k scroll  / find  tab link  f links  o outline  ? help  q quit  {theme_label}"
            ),
            format!("j/k  /  tab  f  o  ?  q  {theme_label}"),
            format!("?  q  {theme_label}"),
            theme_label.to_string(),
        ]
    };
    let left = app.status_left();
    let width = area.width as usize;
    let right = pick_hint(UnicodeWidthStr::width(left.as_str()), width, &hints);
    let right_w = UnicodeWidthStr::width(right.as_str());
    let left_budget = width.saturating_sub(right_w + if right.is_empty() { 0 } else { 2 });
    let left = truncate(&left, left_budget);
    let left_w = UnicodeWidthStr::width(left.as_str());
    let gap = width.saturating_sub(left_w + right_w.min(width));
    let gap_s = " ".repeat(gap);
    let right = if left_w + 2 >= width {
        String::new()
    } else {
        truncate(&right, width - left_w - gap)
    };
    buf.set_line(
        area.x,
        area.y,
        &Line::from(vec![
            Span::styled(left, accent),
            Span::styled(gap_s, style),
            Span::styled(right, style),
        ]),
        area.width,
    );
}

fn draw_help(frame: &mut Frame, _app: &App, theme: &Theme) {
    let area = centered(frame.area(), 62, 20);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" keys ")
        .border_style(Style::default().fg(paint_color(theme.popup_border)))
        .style(
            Style::default()
                .bg(paint_color(theme.popup_bg))
                .fg(paint_color(theme.popup_fg)),
        );
    frame.render_widget(Clear, area);
    let text = "\
j k  ↓ ↑        scroll lines
d u  Ctrl-d/u   half page
space  PgDn     page down
b  PgUp         page up
gg  Home        top
G  End          bottom
h l  ← →        scroll sideways
0               reset sideways scroll
/               search    n N next/prev
[ ]             previous / next heading
o               toggle outline
tab  shift-tab  focus next / previous link
enter           open the focused link
f               link picker
click           open a link
wheel           scroll
backspace       back to the previous file
t               cycle theme (ink, paper, terminal)
?               this help
q  Esc          quit (Esc closes this first)";
    frame.render_widget(
        Paragraph::new(text).block(block).wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_picker(frame: &mut Frame, app: &App, theme: &Theme) {
    let height = (app.rendered.links.len() as u16 + 4).clamp(6, 18);
    let area = centered(frame.area(), 68, height);
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" links ")
        .border_style(Style::default().fg(paint_color(theme.popup_border)))
        .style(
            Style::default()
                .bg(paint_color(theme.popup_bg))
                .fg(paint_color(theme.popup_fg)),
        );
    let inner_width = area.width.saturating_sub(2) as usize;
    let mut lines = Vec::new();
    lines.push(Line::from("enter open   1-9 jump   esc close"));
    let window = (height as usize).saturating_sub(4).max(1);
    let start = app.picker_sel.saturating_sub(window / 2);
    for (offset, link) in app
        .rendered
        .links
        .iter()
        .enumerate()
        .skip(start)
        .take(window)
    {
        let marker = if offset == app.picker_sel { "▸" } else { " " };
        let n = offset + 1;
        let label = truncate(&link.label, inner_width / 2);
        let url = truncate(
            &link.url,
            inner_width.saturating_sub(UnicodeWidthStr::width(label.as_str()) + 8),
        );
        let style = if offset == app.picker_sel {
            Style::default()
                .bg(paint_color(theme.outline_sel_bg))
                .fg(paint_color(theme.link))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(paint_color(theme.popup_fg))
        };
        lines.push(Line::from(Span::styled(
            format!("{marker} {n:>2}  {label}  {url}"),
            style,
        )));
    }
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

pub fn to_ansi(rendered: &Rendered, theme: &Theme) -> String {
    let mut out = String::new();
    for line in &rendered.lines {
        out.push_str(&line_ansi(line, theme, &rendered.links));
        out.push('\n');
    }
    out.push_str("\x1b[0m");
    out
}

fn line_ansi(line: &DocLine, theme: &Theme, links: &[LinkInfo]) -> String {
    let mut out = String::new();
    for seg in &line.spans {
        let mut face = theme.face(seg.style);
        if line.code && face.bg.is_none() {
            face.bg = Some(theme.code_bg);
        }
        out.push_str(&face_to_sgr(&face));
        if let Some(id) = seg.link {
            if let Some(link) = links.iter().find(|link| link.id == id) {
                out.push_str(&osc8_open(&link.url));
                out.push_str(&seg.text);
                out.push_str("\x1b]8;;\x1b\\");
                continue;
            }
        }
        out.push_str(&seg.text);
    }
    out.push_str("\x1b[0m");
    out
}

pub fn paint_osc8(hits: &[HitBox]) -> io::Result<()> {
    if hits.is_empty() {
        return Ok(());
    }
    let mut out = stdout();
    for hit in hits {
        if hit.text.is_empty() || hit.url.is_empty() {
            continue;
        }
        write!(
            out,
            "\x1b[{};{}H{}\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\\x1b[0m",
            hit.y + 1,
            hit.x + 1,
            hit.sgr,
            osc8_url(&hit.url),
            hit.text
        )?;
    }
    write!(out, "\x1b[?25l")?;
    out.flush()
}

fn osc8_open(url: &str) -> String {
    format!("\x1b]8;;{}\x1b\\", osc8_url(url))
}

fn osc8_url(url: &str) -> String {
    let mut out = String::new();
    for byte in url.bytes() {
        if byte.is_ascii_graphic() && byte != b'\\' {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn face_style(theme: &Theme, id: StyleId, fill: bool) -> Style {
    let mut face = theme.face(id);
    if fill && face.bg.is_none() && theme.fill_bg {
        face.bg = Some(theme.bg);
    }
    face_to_style(face)
}

fn face_to_style(face: Face) -> Style {
    let mut style = Style::default().fg(paint_color(face.fg));
    if let Some(bg) = face.bg {
        style = style.bg(paint_color(bg));
    }
    let mut mods = Modifier::empty();
    if face.bold {
        mods |= Modifier::BOLD;
    }
    if face.italic {
        mods |= Modifier::ITALIC;
    }
    if face.underline {
        mods |= Modifier::UNDERLINED;
    }
    if face.strike {
        mods |= Modifier::CROSSED_OUT;
    }
    style.add_modifier(mods)
}

fn style_from_face(face: Face) -> Style {
    face_to_style(face)
}

fn face_to_sgr(face: &Face) -> String {
    let mut parts = Vec::new();
    match face.fg {
        Paint::Rgb(r, g, b) => parts.push(format!("38;2;{r};{g};{b}")),
        Paint::Ansi(n) => parts.push(ansi_color(n, true)),
        Paint::Reset => parts.push("39".into()),
    }
    if let Some(bg) = face.bg {
        match bg {
            Paint::Rgb(r, g, b) => parts.push(format!("48;2;{r};{g};{b}")),
            Paint::Ansi(n) => parts.push(ansi_color(n, false)),
            Paint::Reset => parts.push("49".into()),
        }
    }
    if face.bold {
        parts.push("1".into());
    }
    if face.italic {
        parts.push("3".into());
    }
    if face.underline {
        parts.push("4".into());
    }
    if face.strike {
        parts.push("9".into());
    }
    format!("\x1b[0;{}m", parts.join(";"))
}

fn ansi_color(n: u8, foreground: bool) -> String {
    if n < 8 {
        let base = if foreground { 30 } else { 40 };
        format!("{}", base + n)
    } else if n < 16 {
        let base = if foreground { 90 } else { 100 };
        format!("{}", base + (n - 8))
    } else {
        let prefix = if foreground { 38 } else { 48 };
        format!("{prefix};5;{n}")
    }
}

fn paint_color(paint: Paint) -> Color {
    match paint {
        Paint::Rgb(r, g, b) => Color::Rgb(r, g, b),
        Paint::Ansi(n) => Color::Indexed(n),
        Paint::Reset => Color::Reset,
    }
}

fn truncate(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if UnicodeWidthStr::width(text) <= width {
        return text.to_string();
    }
    let mut out = String::new();
    let mut used = 0usize;
    for grapheme in text.graphemes(true) {
        let w = UnicodeWidthStr::width(grapheme);
        if used + w + 1 > width {
            break;
        }
        out.push_str(grapheme);
        used += w;
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::hit_link;
    use crate::theme::ThemeName;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    #[test]
    fn renders_heading_code_and_link_hits() {
        let sample = "# Showcase\n\nSee [docs](https://ratatui.rs) now.\n\n```rust\nfn main() {}\n```\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n";
        let mut app = App::new(
            "showcase.md".into(),
            Some(PathBuf::from("showcase.md")),
            sample.into(),
            false,
            ThemeName::Ink,
        );
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let text = buffer_text(terminal.backend().buffer());
        assert!(text.contains("Showcase"));
        assert!(!text.contains("# Showcase"));
        assert!(!text.contains("```"));
        assert!(text.contains("fn"));
        assert!(text.contains("docs"));
        assert!(app.hits.iter().any(|hit| hit.url == "https://ratatui.rs"));
        let hit = app
            .hits
            .iter()
            .find(|hit| hit.url == "https://ratatui.rs")
            .unwrap();
        assert_eq!(hit_link(&app.hits, hit.x, hit.y), Some(hit.link_id));
    }

    #[test]
    fn print_emits_osc8() {
        let rendered = crate::layout::render(
            &crate::parse::parse("Go [there](https://example.com).").blocks,
            40,
        );
        let ansi = to_ansi(&rendered, &Theme::get(ThemeName::Ink));
        assert!(ansi.contains("\x1b]8;;https://example.com"));
        assert!(ansi.contains("there"));
        assert!(!ansi.contains("](https://example.com)"));
    }

    fn buffer_text(buf: &ratatui::buffer::Buffer) -> String {
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }
}
