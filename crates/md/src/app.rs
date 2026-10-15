//! Viewer state: scroll, search, history, and link activation. Input is applied
//! here so the terminal loop stays thin and the behavior can be tested.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::layout::{render, Heading, Rendered};
use crate::open::{is_markdown, open_external, resolve_link, slugify, Target};
use crate::parse::{parse, strip_front_matter};
use crate::theme::ThemeName;

#[derive(Clone, Debug)]
pub struct HitBox {
    pub x: u16,
    pub y: u16,
    pub width: usize,
    pub text: String,
    pub url: String,
    pub link_id: u32,
    pub sgr: String,
}

#[derive(Clone, Debug)]
pub struct HeadingHit {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub index: usize,
}

#[derive(Clone, Debug)]
pub struct LineMatches {
    pub spans: Vec<(usize, usize)>,
    pub current: Option<(usize, usize)>,
}

#[derive(Clone, Debug)]
pub struct SearchHit {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug)]
struct Hist {
    title: String,
    path: Option<PathBuf>,
    text: String,
    scroll: usize,
    hscroll: usize,
}

pub struct App {
    pub title: String,
    pub path: Option<PathBuf>,
    pub blocks: Vec<crate::parse::Block>,
    pub rendered: Rendered,
    pub theme_name: ThemeName,
    pub laid_out_width: usize,
    pub scroll: usize,
    pub hscroll: usize,
    pub outline: bool,
    pub query: String,
    pub searching: bool,
    pub matches: Vec<SearchHit>,
    pub match_idx: usize,
    pub focus_link: Option<u32>,
    pub hover_link: Option<u32>,
    pub hits: Vec<HitBox>,
    pub heading_hits: Vec<HeadingHit>,
    pub picker: bool,
    pub picker_sel: usize,
    pub help: bool,
    pub pending_g: bool,
    pub message: Option<String>,
    pub quit: bool,
    pub last_view_rows: usize,
    history: Vec<Hist>,
    raw: String,
}

pub enum KeyInput {
    Char(char),
    Backspace,
    Enter,
    Esc,
    Tab,
    BackTab,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Ctrl(char),
}

impl App {
    pub fn new(
        title: String,
        path: Option<PathBuf>,
        text: String,
        lossy: bool,
        theme: ThemeName,
    ) -> Self {
        let text = strip_front_matter(&text);
        let blocks = parse(&text).blocks;
        let rendered = render(&blocks, 80);
        let mut app = Self {
            title,
            path,
            raw: text,
            blocks,
            rendered,
            theme_name: theme,
            laid_out_width: 80,
            scroll: 0,
            hscroll: 0,
            outline: false,
            query: String::new(),
            searching: false,
            matches: Vec::new(),
            match_idx: 0,
            focus_link: None,
            hover_link: None,
            hits: Vec::new(),
            heading_hits: Vec::new(),
            picker: false,
            picker_sel: 0,
            help: false,
            pending_g: false,
            message: None,
            quit: false,
            last_view_rows: 24,
            history: Vec::new(),
        };
        if lossy {
            app.message = Some("invalid UTF-8; replacement characters inserted".into());
        }
        app
    }

    pub fn relayout(&mut self, width: usize) {
        let width = width.max(1);
        if width == self.laid_out_width {
            return;
        }
        self.rendered = render(&self.blocks, width);
        self.laid_out_width = width;
        self.clamp_scroll();
        self.recompute_matches();
    }

    pub fn theme(&self) -> crate::theme::Theme {
        crate::theme::Theme::get(self.theme_name)
    }

    pub fn handle_key(&mut self, key: KeyInput) {
        if matches!(key, KeyInput::Ctrl('c')) {
            self.quit = true;
            return;
        }
        if self.searching {
            self.message = None;
            self.edit_search(key);
            return;
        }
        if matches!(key, KeyInput::Char('q')) {
            self.quit = true;
            return;
        }
        self.message = None;
        if self.help {
            if matches!(key, KeyInput::Esc | KeyInput::Char('?')) {
                self.help = false;
            }
            return;
        }
        if self.picker {
            self.picker_key(key);
            return;
        }
        if self.pending_g {
            self.pending_g = false;
            if matches!(key, KeyInput::Char('g')) {
                self.go_top();
                return;
            }
        }
        if matches!(key, KeyInput::Esc) {
            self.quit = true;
            return;
        }
        match key {
            KeyInput::Char('j') | KeyInput::Down => self.scroll_by(1),
            KeyInput::Char('k') | KeyInput::Up => self.scroll_by(-1),
            KeyInput::Char('d') | KeyInput::Ctrl('d') => self.scroll_by(self.half_page() as isize),
            KeyInput::Char('u') | KeyInput::Ctrl('u') => {
                self.scroll_by(-(self.half_page() as isize))
            }
            KeyInput::Char(' ') | KeyInput::PageDown | KeyInput::Ctrl('f') => {
                self.scroll_by(self.page() as isize);
            }
            KeyInput::Char('b') | KeyInput::PageUp => self.scroll_by(-(self.page() as isize)),
            KeyInput::Char('G') | KeyInput::End => self.go_bottom(),
            KeyInput::Home => self.go_top(),
            KeyInput::Char('g') => self.pending_g = true,
            KeyInput::Char('h') | KeyInput::Left => self.hscroll_by(-4),
            KeyInput::Char('l') | KeyInput::Right => self.hscroll_by(4),
            KeyInput::Char('0') => self.hscroll = 0,
            KeyInput::Char('/') => {
                self.searching = true;
                self.picker = false;
            }
            KeyInput::Char('n') => self.step_match(true),
            KeyInput::Char('N') => self.step_match(false),
            KeyInput::Char('t') => self.theme_name = self.theme_name.cycle(),
            KeyInput::Char('o') => self.outline = !self.outline,
            KeyInput::Char('[') => self.jump_heading(false),
            KeyInput::Char(']') => self.jump_heading(true),
            KeyInput::Char('f') => self.toggle_picker(),
            KeyInput::Char('?') => self.help = true,
            KeyInput::Tab => self.cycle_link(true),
            KeyInput::BackTab => self.cycle_link(false),
            KeyInput::Enter => self.open_focused(),
            KeyInput::Backspace => self.go_back(),
            KeyInput::Ctrl('b') => self.go_back(),
            _ => {}
        }
    }

    fn edit_search(&mut self, key: KeyInput) {
        match key {
            KeyInput::Esc => {
                self.searching = false;
                self.query.clear();
                self.matches.clear();
                self.match_idx = 0;
            }
            KeyInput::Enter => {
                self.searching = false;
                self.recompute_matches();
                if self.matches.is_empty() {
                    if !self.query.is_empty() {
                        self.message = Some("no matches".into());
                    }
                } else {
                    self.match_idx = 0;
                    self.reveal_current();
                }
            }
            KeyInput::Backspace => {
                self.query.pop();
                self.recompute_matches();
            }
            KeyInput::Ctrl('u') => {
                self.query.clear();
                self.recompute_matches();
            }
            KeyInput::Char(c) => {
                self.query.push(c);
                self.recompute_matches();
            }
            _ => {}
        }
    }

    fn picker_key(&mut self, key: KeyInput) {
        let len = self.rendered.links.len();
        if len == 0 {
            self.picker = false;
            return;
        }
        match key {
            KeyInput::Esc | KeyInput::Char('f') => self.picker = false,
            KeyInput::Down | KeyInput::Char('j') => {
                self.picker_sel = (self.picker_sel + 1).min(len - 1);
            }
            KeyInput::Up | KeyInput::Char('k') => {
                self.picker_sel = self.picker_sel.saturating_sub(1);
            }
            KeyInput::Tab => self.picker_sel = (self.picker_sel + 1) % len,
            KeyInput::BackTab => {
                self.picker_sel = if self.picker_sel == 0 {
                    len - 1
                } else {
                    self.picker_sel - 1
                };
            }
            KeyInput::Enter => {
                let id = self.rendered.links[self.picker_sel].id;
                self.picker = false;
                self.focus_link = Some(id);
                self.activate_link(id);
            }
            KeyInput::Char(c) if c.is_ascii_digit() && c != '0' => {
                let idx = (c as u8 - b'1') as usize;
                if idx < len {
                    let id = self.rendered.links[idx].id;
                    self.picker = false;
                    self.focus_link = Some(id);
                    self.activate_link(id);
                }
            }
            _ => {}
        }
    }

    pub fn scroll_by(&mut self, delta: isize) {
        let next = self.scroll as isize + delta;
        self.scroll = next.max(0) as usize;
        self.clamp_scroll();
    }

    pub fn hscroll_by(&mut self, delta: isize) {
        let view = self.laid_out_width.max(1);
        let max = self.rendered.max_width.saturating_sub(view);
        let next = self.hscroll as isize + delta;
        self.hscroll = next.clamp(0, max as isize) as usize;
    }

    pub fn go_top(&mut self) {
        self.scroll = 0;
        self.hscroll = 0;
    }

    pub fn go_bottom(&mut self) {
        let view = self.last_view_rows.max(1);
        self.scroll = self.rendered.lines.len().saturating_sub(view);
    }

    pub fn clamp_scroll(&mut self) {
        let view = self.last_view_rows.max(1);
        let max = self.rendered.lines.len().saturating_sub(view);
        if self.scroll > max {
            self.scroll = max;
        }
    }

    fn page(&self) -> usize {
        self.last_view_rows.saturating_sub(1).max(1)
    }

    fn half_page(&self) -> usize {
        (self.last_view_rows / 2).max(1)
    }

    pub fn recompute_matches(&mut self) {
        self.matches.clear();
        if self.query.is_empty() {
            self.match_idx = 0;
            return;
        }
        for (line, doc) in self.rendered.lines.iter().enumerate() {
            find_in_line(&doc.plain, &self.query, line, &mut self.matches);
        }
        if self.match_idx >= self.matches.len() {
            self.match_idx = 0;
        }
    }

    fn step_match(&mut self, forward: bool) {
        if self.matches.is_empty() {
            self.message = Some(if self.query.is_empty() {
                "no search".into()
            } else {
                "no matches".into()
            });
            return;
        }
        if forward {
            self.match_idx = (self.match_idx + 1) % self.matches.len();
        } else {
            self.match_idx = if self.match_idx == 0 {
                self.matches.len() - 1
            } else {
                self.match_idx - 1
            };
        }
        self.reveal_current();
    }

    fn reveal_current(&mut self) {
        if let Some(hit) = self.matches.get(self.match_idx) {
            let line = hit.line;
            self.scroll_into(line);
        }
    }

    pub fn scroll_into(&mut self, line: usize) {
        let view = self.last_view_rows.max(1);
        if line < self.scroll || line >= self.scroll.saturating_add(view) {
            self.scroll = line.saturating_sub(view / 3);
        }
        self.clamp_scroll();
    }

    pub fn jump_heading(&mut self, forward: bool) {
        let headings = &self.rendered.headings;
        if headings.is_empty() {
            self.message = Some("no headings".into());
            return;
        }
        let sel = if forward {
            headings
                .iter()
                .position(|h| h.line > self.scroll)
                .unwrap_or(headings.len() - 1)
        } else {
            headings
                .iter()
                .rposition(|h| h.line < self.scroll)
                .unwrap_or(0)
        };
        self.scroll = headings[sel].line;
        self.clamp_scroll();
    }

    pub fn current_heading(&self) -> Option<&Heading> {
        self.rendered
            .headings
            .iter()
            .rfind(|h| h.line <= self.scroll)
    }

    pub fn outline_index(&self) -> usize {
        self.rendered
            .headings
            .iter()
            .rposition(|h| h.line <= self.scroll)
            .unwrap_or(0)
    }

    fn toggle_picker(&mut self) {
        if self.rendered.links.is_empty() {
            self.message = Some("no links".into());
            return;
        }
        self.picker = !self.picker;
        if self.picker {
            self.picker_sel = self
                .focus_link
                .and_then(|id| self.rendered.links.iter().position(|l| l.id == id))
                .unwrap_or(0);
        }
    }

    pub fn cycle_link(&mut self, forward: bool) {
        let links = &self.rendered.links;
        if links.is_empty() {
            self.message = Some("no links".into());
            return;
        }
        let idx = self
            .focus_link
            .and_then(|id| links.iter().position(|l| l.id == id));
        let next = match (idx, forward) {
            (Some(i), true) => (i + 1) % links.len(),
            (Some(0), false) | (None, false) => links.len() - 1,
            (Some(i), false) => i - 1,
            (None, true) => 0,
        };
        let line = links[next].first_line;
        let id = links[next].id;
        self.focus_link = Some(id);
        self.scroll_into(line);
    }

    fn open_focused(&mut self) {
        if let Some(id) = self.focus_link {
            self.activate_link(id);
        } else if let Some(link) = self.rendered.links.first() {
            let id = link.id;
            self.focus_link = Some(id);
            self.scroll_into(link.first_line);
        } else {
            self.message = Some("no links".into());
        }
    }

    pub fn activate_link(&mut self, id: u32) {
        let Some(link) = self
            .rendered
            .links
            .iter()
            .find(|link| link.id == id)
            .cloned()
        else {
            return;
        };
        self.focus_link = Some(id);
        self.follow(&link.url);
    }

    pub fn follow(&mut self, url: &str) {
        match resolve_link(self.path.as_deref(), url) {
            Target::Browser(url) => match open_external(&url) {
                Ok(()) => self.message = Some(format!("opened {url}")),
                Err(err) => self.message = Some(err),
            },
            Target::Anchor(slug) => {
                if !self.jump_slug(&slug) {
                    self.message = Some(format!("no heading #{slug}"));
                }
            }
            Target::File { path, anchor } => self.open_path(path, anchor),
            Target::Unsupported(msg) => self.message = Some(msg),
        }
    }

    fn open_path(&mut self, path: PathBuf, anchor: Option<String>) {
        if path.is_dir() {
            self.message = Some(format!("{} is a directory", path.display()));
            return;
        }
        if !is_markdown(&path) {
            if path.exists() {
                let shown = path.display().to_string();
                match open_external(&shown) {
                    Ok(()) => self.message = Some(format!("opened {shown}")),
                    Err(err) => self.message = Some(err),
                }
            } else {
                self.message = Some(format!("cannot read {}: not found", path.display()));
            }
            return;
        }
        match read_path(&path) {
            Ok((text, lossy)) => {
                self.push_history();
                let title = path.display().to_string();
                self.load_doc(title, Some(path), text, lossy);
                if let Some(anchor) = anchor {
                    if !self.jump_slug(&anchor) {
                        self.message = Some(format!("opened file; no heading #{anchor}"));
                    }
                }
            }
            Err(err) => self.message = Some(err),
        }
    }

    fn load_doc(&mut self, title: String, path: Option<PathBuf>, text: String, lossy: bool) {
        let text = strip_front_matter(&text);
        self.title = title;
        self.path = path;
        self.raw = text;
        self.blocks = parse(&self.raw).blocks;
        self.scroll = 0;
        self.hscroll = 0;
        self.focus_link = None;
        self.hover_link = None;
        self.query.clear();
        self.searching = false;
        self.matches.clear();
        self.match_idx = 0;
        self.picker = false;
        self.help = false;
        self.pending_g = false;
        let width = self.laid_out_width.max(1);
        self.laid_out_width = 0;
        self.relayout(width);
        if lossy {
            self.message = Some("invalid UTF-8; replacement characters inserted".into());
        }
    }

    fn push_history(&mut self) {
        self.history.push(Hist {
            title: self.title.clone(),
            path: self.path.clone(),
            text: self.raw.clone(),
            scroll: self.scroll,
            hscroll: self.hscroll,
        });
        if self.history.len() > 50 {
            self.history.remove(0);
        }
    }

    pub fn go_back(&mut self) {
        let Some(prev) = self.history.pop() else {
            self.message = Some("nothing to go back to".into());
            return;
        };
        let scroll = prev.scroll;
        let hscroll = prev.hscroll;
        self.load_doc(prev.title, prev.path, prev.text, false);
        self.scroll = scroll;
        self.hscroll = hscroll;
        self.clamp_scroll();
    }

    pub fn jump_slug(&mut self, slug: &str) -> bool {
        let want = slugify(slug);
        let line = self
            .rendered
            .anchors
            .iter()
            .find(|anchor| anchor.slug == want || anchor.slug == slug)
            .map(|anchor| anchor.line)
            .or_else(|| {
                self.rendered
                    .headings
                    .iter()
                    .find(|h| h.slug == want || h.text.eq_ignore_ascii_case(slug))
                    .map(|h| h.line)
            });
        if let Some(line) = line {
            self.scroll = line;
            self.clamp_scroll();
            true
        } else {
            false
        }
    }

    pub fn hover_at(&mut self, x: u16, y: u16) -> bool {
        if self.help || self.picker {
            return false;
        }
        let id = hit_link(&self.hits, x, y);
        if id != self.hover_link {
            self.hover_link = id;
            true
        } else {
            false
        }
    }

    pub fn click_at(&mut self, x: u16, y: u16) {
        if self.help {
            self.help = false;
            return;
        }
        if self.picker {
            self.picker = false;
            return;
        }
        if let Some(hit) = self
            .heading_hits
            .iter()
            .find(|hit| y == hit.y && x >= hit.x && x < hit.x.saturating_add(hit.width))
        {
            if let Some(heading) = self.rendered.headings.get(hit.index) {
                self.scroll = heading.line;
                self.clamp_scroll();
            }
            return;
        }
        if let Some(id) = hit_link(&self.hits, x, y) {
            self.focus_link = Some(id);
            self.activate_link(id);
        }
    }

    pub fn match_span(&self, line: usize) -> LineMatches {
        let mut spans = Vec::new();
        let mut current = None;
        for (idx, hit) in self.matches.iter().enumerate() {
            if hit.line == line {
                spans.push((hit.start, hit.end));
                if idx == self.match_idx && !self.searching {
                    current = Some((hit.start, hit.end));
                }
            }
        }
        LineMatches { spans, current }
    }

    pub fn status_left(&self) -> String {
        if let Some(message) = &self.message {
            return message.clone();
        }
        if let Some(id) = self.hover_link.or(self.focus_link) {
            if let Some(link) = self.rendered.links.iter().find(|link| link.id == id) {
                return link.url.clone();
            }
        }
        let pct = percent(self.scroll, self.rendered.lines.len(), self.last_view_rows);
        let heading = self
            .current_heading()
            .map(|h| h.text.as_str())
            .unwrap_or("");
        if heading.is_empty() {
            format!("{}  {pct}%", file_name(&self.title))
        } else {
            format!("{}  {pct}%  {heading}", file_name(&self.title))
        }
    }

    pub fn search_label(&self) -> String {
        if self.matches.is_empty() {
            format!("/ {}_", self.query)
        } else if self.searching {
            format!("/ {}_   {} matches", self.query, self.matches.len())
        } else {
            format!(
                "/ {}   {}/{}",
                self.query,
                self.match_idx + 1,
                self.matches.len()
            )
        }
    }
}

pub fn hit_link(hits: &[HitBox], x: u16, y: u16) -> Option<u32> {
    hits.iter().find_map(|hit| {
        let end = hit.x as usize + hit.width;
        if y == hit.y && x as usize >= hit.x as usize && (x as usize) < end {
            Some(hit.link_id)
        } else {
            None
        }
    })
}

fn find_in_line(text: &str, query: &str, line: usize, out: &mut Vec<SearchHit>) {
    if query.is_empty() {
        return;
    }
    let fold = query.is_ascii() && query.bytes().all(|b| !b.is_ascii_uppercase());
    let (hay, needle) = if fold {
        (text.to_ascii_lowercase(), query.to_ascii_lowercase())
    } else {
        (text.to_string(), query.to_string())
    };
    let mut start = 0;
    while let Some(rel) = hay[start..].find(&needle) {
        let begin = start + rel;
        let end = begin + needle.len();
        if text.is_char_boundary(begin) && text.is_char_boundary(end) {
            let start_col = unicode_width::UnicodeWidthStr::width(&text[..begin]);
            let end_col = unicode_width::UnicodeWidthStr::width(&text[..end]);
            if end_col > start_col {
                out.push(SearchHit {
                    line,
                    start: start_col,
                    end: end_col,
                });
            }
        }
        start = begin + needle.len().max(1);
        if start >= hay.len() {
            break;
        }
    }
}

fn percent(scroll: usize, len: usize, view: usize) -> u16 {
    if len <= view.max(1) {
        100
    } else {
        let max = len - view.max(1);
        if scroll >= max {
            100
        } else {
            ((scroll * 100) / max.max(1)) as u16
        }
    }
}

fn file_name(title: &str) -> String {
    Path::new(title)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| title.to_string())
}

pub fn read_path(path: &Path) -> Result<(String, bool), String> {
    let meta =
        fs::metadata(path).map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    if meta.is_dir() {
        return Err(format!("{} is a directory", path.display()));
    }
    let bytes = fs::read(path).map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    Ok(decode_bytes(bytes))
}

pub fn read_stdin() -> Result<(String, bool), String> {
    let mut bytes = Vec::new();
    io::stdin()
        .read_to_end(&mut bytes)
        .map_err(|err| format!("cannot read stdin: {err}"))?;
    Ok(decode_bytes(bytes))
}

fn decode_bytes(bytes: Vec<u8>) -> (String, bool) {
    match String::from_utf8(bytes) {
        Ok(text) => (text, false),
        Err(err) => (String::from_utf8_lossy(err.as_bytes()).into_owned(), true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_and_scroll() {
        let mut app = App::new(
            "demo.md".into(),
            Some(PathBuf::from("demo.md")),
            "# Hello\n\nalpha beta alpha\n\n## Later\n".into(),
            false,
            ThemeName::Ink,
        );
        app.last_view_rows = 2;
        app.handle_key(KeyInput::Char('/'));
        for c in ['a', 'l', 'p', 'h', 'a'] {
            app.handle_key(KeyInput::Char(c));
        }
        app.handle_key(KeyInput::Enter);
        assert!(app.matches.len() >= 2);
        let first = app.matches[0].line;
        app.handle_key(KeyInput::Char('n'));
        assert_ne!(app.scroll, 0);
        let _ = first;
        app.handle_key(KeyInput::Char('g'));
        app.handle_key(KeyInput::Char('g'));
        assert_eq!(app.scroll, 0);
        app.handle_key(KeyInput::Char(']'));
        assert!(app.scroll > 0);
    }

    #[test]
    fn relative_link_back_navigation() {
        let dir = std::env::temp_dir().join(format!("md-link-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.md");
        let b = dir.join("b.md");
        fs::write(&a, "See [notes](b.md) please.\n").unwrap();
        fs::write(&b, "# Bee\n\nBack to [start](a.md).\n").unwrap();
        let (text, _) = read_path(&a).unwrap();
        let mut app = App::new(
            a.display().to_string(),
            Some(a.clone()),
            text,
            false,
            ThemeName::Ink,
        );
        let id = app.rendered.links[0].id;
        app.activate_link(id);
        assert!(app.rendered.headings.iter().any(|h| h.text == "Bee"));
        app.handle_key(KeyInput::Backspace);
        assert!(app.path.as_deref() == Some(a.as_path()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_message() {
        let err = read_path(Path::new("no-such-md-file.md")).unwrap_err();
        assert!(err.contains("cannot read"));
    }
}
