//! Wrap a parsed document to a terminal width. Lines keep semantic styles and
//! link ids; the theme and hit testing happen later, at draw time.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::highlight::{highlight_line, HlState};
use crate::open::slugify;
use crate::parse::{inline_plain, Align, Block, Inline, QuoteKind};
use crate::theme::StyleId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seg {
    pub text: String,
    pub style: StyleId,
    pub link: Option<u32>,
}

impl Seg {
    pub fn new(text: impl Into<String>, style: StyleId) -> Self {
        Self {
            text: text.into(),
            style,
            link: None,
        }
    }

    pub fn linked(text: impl Into<String>, style: StyleId, link: Option<u32>) -> Self {
        Self {
            text: text.into(),
            style,
            link,
        }
    }

    fn spaces(n: usize, style: StyleId) -> Self {
        Self::new(" ".repeat(n), style)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocLine {
    pub spans: Vec<Seg>,
    pub plain: String,
    pub code: bool,
}

impl DocLine {
    fn from_spans(spans: Vec<Seg>, code: bool) -> Self {
        let plain = spans.iter().map(|seg| seg.text.as_str()).collect();
        Self { spans, plain, code }
    }

    pub fn is_blank(&self) -> bool {
        self.plain.trim().is_empty()
    }

    pub fn width(&self) -> usize {
        UnicodeWidthStr::width(self.plain.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub slug: String,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Anchor {
    pub slug: String,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkInfo {
    pub id: u32,
    pub url: String,
    pub label: String,
    pub first_line: usize,
    placed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rendered {
    pub lines: Vec<DocLine>,
    pub headings: Vec<Heading>,
    pub anchors: Vec<Anchor>,
    pub links: Vec<LinkInfo>,
    pub max_width: usize,
}

pub fn render(blocks: &[Block], width: usize) -> Rendered {
    let width = width.max(1);
    let mut lay = Lay::new();
    layout_blocks(blocks, &mut lay, &[], width, false);
    while lay
        .lines
        .last()
        .is_some_and(|line| line.is_blank() && !line.code)
    {
        lay.lines.pop();
    }
    let max_width = lay.lines.iter().map(DocLine::width).max().unwrap_or(0);
    Rendered {
        lines: lay.lines,
        headings: lay.headings,
        anchors: lay.anchors,
        links: lay.links,
        max_width,
    }
}

struct Lay {
    lines: Vec<DocLine>,
    headings: Vec<Heading>,
    anchors: Vec<Anchor>,
    links: Vec<LinkInfo>,
    next_link: u32,
    slugs: Vec<String>,
}

impl Lay {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            headings: Vec::new(),
            anchors: Vec::new(),
            links: Vec::new(),
            next_link: 1,
            slugs: Vec::new(),
        }
    }

    fn push(&mut self, spans: Vec<Seg>, code: bool) {
        let line_no = self.lines.len();
        for seg in &spans {
            if let Some(id) = seg.link {
                if let Some(info) = self.links.iter_mut().find(|link| link.id == id) {
                    if !info.placed {
                        info.first_line = line_no;
                        info.placed = true;
                    }
                }
            }
        }
        self.lines.push(DocLine::from_spans(spans, code));
    }

    fn push_blank(&mut self) {
        if self.lines.last().is_some_and(DocLine::is_blank) || self.lines.is_empty() {
            return;
        }
        self.lines.push(DocLine::from_spans(Vec::new(), false));
    }

    fn alloc_link(&mut self, url: &str, label: &str) -> u32 {
        let id = self.next_link;
        self.next_link += 1;
        let label = if label.trim().is_empty() {
            url.to_string()
        } else {
            label.trim().to_string()
        };
        self.links.push(LinkInfo {
            id,
            url: url.to_string(),
            label,
            first_line: self.lines.len(),
            placed: false,
        });
        id
    }

    fn alloc_slug(&mut self, text: &str) -> String {
        let base = slugify(text);
        let base = if base.is_empty() {
            "section".to_string()
        } else {
            base
        };
        let mut slug = base.clone();
        let mut n = 1;
        while self.slugs.iter().any(|existing| existing == &slug) {
            n += 1;
            slug = format!("{base}-{n}");
        }
        self.slugs.push(slug.clone());
        slug
    }
}

fn layout_blocks(blocks: &[Block], lay: &mut Lay, prefix: &[Seg], width: usize, tight: bool) {
    for (index, block) in blocks.iter().enumerate() {
        if index > 0 && !tight {
            lay.push_blank();
        }
        layout_block(block, lay, prefix, width);
    }
}

fn layout_block(block: &Block, lay: &mut Lay, prefix: &[Seg], width: usize) {
    match block {
        Block::Heading { level, inlines } => layout_heading(*level, inlines, lay, prefix, width),
        Block::Paragraph(inlines) => {
            let style = if quote_prefix(prefix) {
                StyleId::Quote
            } else {
                StyleId::Text
            };
            layout_inlines(inlines, lay, prefix, prefix, width, style, false);
        }
        Block::Code { lang, code } => layout_code(lang.as_deref(), code, lay, prefix, width),
        Block::Quote { kind, blocks } => layout_quote(*kind, blocks, lay, prefix, width),
        Block::List {
            ordered,
            start,
            items,
        } => layout_list(*ordered, *start, items, lay, prefix, width),
        Block::Table {
            alignments,
            header,
            rows,
        } => layout_table(alignments, header, rows, lay, prefix, width),
        Block::Rule => {
            let inner = width.saturating_sub(segs_width(prefix)).max(1);
            let mut spans = prefix.to_vec();
            spans.push(Seg::new("\u{2500}".repeat(inner), StyleId::Rule));
            lay.push(spans, false);
        }
        Block::Html(html) => {
            let text = html.split_whitespace().collect::<Vec<_>>().join(" ");
            layout_plain(&text, StyleId::Html, lay, prefix, width, false);
        }
        Block::Footnote { label, blocks } => {
            let anchor = format!("fn-{}", slugify(label));
            lay.anchors.push(Anchor {
                slug: anchor,
                line: lay.lines.len(),
            });
            let mut spans = prefix.to_vec();
            spans.push(Seg::new(format!("[{label}]"), StyleId::Link));
            lay.push(spans, false);
            let mut hang = prefix.to_vec();
            hang.push(Seg::spaces(2, StyleId::Text));
            layout_blocks(blocks, lay, &hang, width, true);
        }
    }
}

fn layout_heading(level: u8, inlines: &[Inline], lay: &mut Lay, prefix: &[Seg], width: usize) {
    let text = inline_plain(inlines);
    let slug = lay.alloc_slug(&text);
    let line = lay.lines.len();
    lay.headings.push(Heading {
        level,
        text: text.trim().to_string(),
        slug: slug.clone(),
        line,
    });
    lay.anchors.push(Anchor { slug, line });
    let style = match level {
        1 => StyleId::H1,
        2 => StyleId::H2,
        3 => StyleId::H3,
        4 => StyleId::H4,
        5 => StyleId::H5,
        _ => StyleId::H6,
    };
    let before = lay.lines.len();
    layout_inlines(inlines, lay, prefix, prefix, width, style, false);
    // VS Code draws a full-width border under h1 and h2 only. h1 uses the
    // heavier rule. h3 (1.25em) gets a rule only as wide as the title, so it
    // sits between a section rule and a plain h4. h4\u{2013}h6 have no rule.
    let inner = width.saturating_sub(segs_width(prefix)).max(1);
    match level {
        1 => push_rule(lay, prefix, inner, "\u{2501}", style),
        2 => push_rule(lay, prefix, inner, "\u{2500}", StyleId::Rule),
        3 => {
            let prefix_w = segs_width(prefix);
            let text_w = lay.lines[before..]
                .iter()
                .map(|line| line.width().saturating_sub(prefix_w))
                .max()
                .unwrap_or(0);
            if text_w > 0 {
                push_rule(lay, prefix, text_w.min(inner), "\u{2500}", StyleId::Rule);
            }
        }
        _ => {}
    }
}

fn push_rule(lay: &mut Lay, prefix: &[Seg], len: usize, glyph: &str, style: StyleId) {
    let mut spans = prefix.to_vec();
    spans.push(Seg::new(glyph.repeat(len.max(1)), style));
    lay.push(spans, false);
}

fn layout_quote(
    kind: Option<QuoteKind>,
    blocks: &[Block],
    lay: &mut Lay,
    prefix: &[Seg],
    width: usize,
) {
    let mut bar = prefix.to_vec();
    bar.push(Seg::new("\u{258c} ", StyleId::QuoteBar));
    if let Some(kind) = kind {
        let mut label = bar.clone();
        label.push(Seg::new(kind.label().to_string(), StyleId::Alert));
        lay.push(label, false);
    }
    layout_blocks(blocks, lay, &bar, width, false);
}

fn layout_list(
    ordered: bool,
    start: u64,
    items: &[crate::parse::ListItem],
    lay: &mut Lay,
    prefix: &[Seg],
    width: usize,
) {
    let last = start.saturating_add(items.len().saturating_sub(1) as u64);
    let num_width = last.to_string().len();
    for (index, item) in items.iter().enumerate() {
        let marker = if let Some(done) = item.checked {
            if done {
                "\u{2713} ".to_string()
            } else {
                "\u{25cb} ".to_string()
            }
        } else if ordered {
            format!("{:width$}. ", start as usize + index, width = num_width)
        } else {
            "\u{2022} ".to_string()
        };
        let marker_style = match item.checked {
            Some(true) => StyleId::TaskDone,
            Some(false) => StyleId::TaskOpen,
            None => StyleId::ListMarker,
        };
        let marker_w = UnicodeWidthStr::width(marker.as_str());
        let mut first = prefix.to_vec();
        first.push(Seg::new(marker, marker_style));
        let mut hang = prefix.to_vec();
        hang.push(Seg::spaces(marker_w, StyleId::Text));

        if item.blocks.is_empty() {
            lay.push(first, false);
            continue;
        }
        let mut blocks = item.blocks.as_slice();
        if let Some(Block::Paragraph(inlines)) = blocks.first() {
            let style = if quote_prefix(prefix) {
                StyleId::Quote
            } else {
                StyleId::Text
            };
            layout_inlines(inlines, lay, &first, &hang, width, style, false);
            blocks = &blocks[1..];
        } else {
            lay.push(first, false);
        }
        layout_blocks(blocks, lay, &hang, width, true);
    }
}

fn layout_code(lang: Option<&str>, code: &str, lay: &mut Lay, prefix: &[Seg], _width: usize) {
    // Vertical padding, same idea as VS Code's 16px pre padding, without a
    // full-width rule that would read as another heading underline.
    lay.push(prefix.to_vec(), true);
    if let Some(lang) = lang {
        if !lang.is_empty() {
            let mut spans = prefix.to_vec();
            spans.push(Seg::new("\u{2502} ", StyleId::QuoteBar));
            spans.push(Seg::new(lang.to_string(), StyleId::Lang));
            lay.push(spans, true);
        }
    }
    let raw_lines: Vec<&str> = if code.is_empty() {
        vec![""]
    } else {
        code.split('\n').collect()
    };
    let digits = digits(raw_lines.len());
    let mut state = HlState::default();
    let lang_key = lang.unwrap_or("");
    for (index, line) in raw_lines.iter().enumerate() {
        let mut spans = prefix.to_vec();
        spans.push(Seg::new(
            format!("{:>width$} ", index + 1, width = digits),
            StyleId::Gutter,
        ));
        spans.push(Seg::new("\u{2502} ", StyleId::QuoteBar));
        let expanded = line.replace('\t', "    ");
        for (text, style) in highlight_line(lang_key, &expanded, &mut state) {
            if text.is_empty() {
                continue;
            }
            spans.push(Seg::new(text, style));
        }
        lay.push(spans, true);
    }
    lay.push(prefix.to_vec(), true);
}

fn layout_table(
    alignments: &[Align],
    header: &[Vec<Inline>],
    rows: &[Vec<Vec<Inline>>],
    lay: &mut Lay,
    prefix: &[Seg],
    width: usize,
) {
    let cols = header
        .len()
        .max(rows.iter().map(Vec::len).max().unwrap_or(0));
    if cols == 0 {
        return;
    }
    let mut naturals = vec![0usize; cols];
    let measure = |cells: &[Vec<Inline>], naturals: &mut [usize]| {
        for (index, cell) in cells.iter().enumerate() {
            if index >= naturals.len() {
                break;
            }
            let w = UnicodeWidthStr::width(inline_plain(cell).as_str());
            naturals[index] = naturals[index].max(w);
        }
    };
    measure(header, &mut naturals);
    for row in rows {
        measure(row, &mut naturals);
    }
    let inner = width.saturating_sub(segs_width(prefix)).max(1);
    let gap = 2usize;
    let widths = distribute(&naturals, inner, gap);
    let header_lines = cells_to_lines(header, &widths, alignments, cols, StyleId::TableHeader, lay);
    push_row_lines(&header_lines, lay, prefix);
    let rule_width = widths.iter().sum::<usize>() + gap * widths.len().saturating_sub(1);
    let mut rule = prefix.to_vec();
    rule.push(Seg::new("\u{2500}".repeat(rule_width.max(1)), StyleId::Rule));
    lay.push(rule, false);
    for row in rows {
        let lines = cells_to_lines(row, &widths, alignments, cols, StyleId::TableCell, lay);
        push_row_lines(&lines, lay, prefix);
    }
}

fn cells_to_lines(
    cells: &[Vec<Inline>],
    widths: &[usize],
    alignments: &[Align],
    cols: usize,
    fallback: StyleId,
    lay: &mut Lay,
) -> Vec<Vec<Vec<Seg>>> {
    let mut wrapped = Vec::new();
    let mut height = 1usize;
    for index in 0..cols {
        let width = widths.get(index).copied().unwrap_or(1).max(1);
        let empty = Vec::new();
        let inlines = cells.get(index).unwrap_or(&empty);
        let atoms = flatten(inlines, lay, fallback);
        let glys = to_glys(&atoms);
        let mut lines = wrap_glys(&glys, width, &[], &[]);
        if lines.is_empty() {
            lines.push(Vec::new());
        }
        height = height.max(lines.len());
        wrapped.push(lines);
    }
    let mut rows = Vec::new();
    for row_index in 0..height {
        let mut row = Vec::new();
        for (col, lines) in wrapped.iter().enumerate() {
            let target = widths.get(col).copied().unwrap_or(1);
            let segs = lines.get(row_index).cloned().unwrap_or_default();
            let align = alignments.get(col).copied().unwrap_or(Align::Left);
            row.push(pad_line(segs, target, align, fallback));
        }
        rows.push(row);
    }
    rows
}

fn push_row_lines(rows: &[Vec<Vec<Seg>>], lay: &mut Lay, prefix: &[Seg]) {
    let gap = Seg::spaces(2, StyleId::TableCell);
    for row in rows {
        let mut spans = prefix.to_vec();
        for (index, cell) in row.iter().enumerate() {
            if index > 0 {
                spans.push(gap.clone());
            }
            spans.extend(cell.clone());
        }
        lay.push(spans, false);
    }
}

fn pad_line(mut segs: Vec<Seg>, target: usize, align: Align, style: StyleId) -> Vec<Seg> {
    let w = segs_width(&segs);
    if w >= target {
        return segs;
    }
    let extra = target - w;
    let (left, right) = match align {
        Align::Right => (extra, 0),
        Align::Center => (extra / 2, extra - extra / 2),
        Align::Left => (0, extra),
    };
    if left > 0 {
        segs.insert(0, Seg::spaces(left, style));
    }
    if right > 0 {
        segs.push(Seg::spaces(right, style));
    }
    segs
}

fn distribute(naturals: &[usize], width: usize, gap: usize) -> Vec<usize> {
    let n = naturals.len();
    if n == 0 {
        return Vec::new();
    }
    let gaps = gap * n.saturating_sub(1);
    let available = width.saturating_sub(gaps).max(n);
    let mut widths: Vec<usize> = naturals.iter().map(|w| (*w).max(1)).collect();
    let mut total: usize = widths.iter().sum();
    if total <= available {
        return widths;
    }
    while total > available {
        let Some(index) = widths
            .iter()
            .enumerate()
            .filter(|(_, w)| **w > 3)
            .max_by_key(|(_, w)| *w)
            .map(|(index, _)| index)
        else {
            break;
        };
        widths[index] -= 1;
        total -= 1;
    }
    widths
}

fn layout_plain(
    text: &str,
    style: StyleId,
    lay: &mut Lay,
    prefix: &[Seg],
    width: usize,
    code: bool,
) {
    let atoms = vec![Atom::Text {
        text: text.to_string(),
        style,
        link: None,
    }];
    let lines = wrap_glys(&to_glys(&atoms), width, prefix, prefix);
    if lines.is_empty() {
        lay.push(prefix.to_vec(), code);
    } else {
        for line in lines {
            lay.push(line, code);
        }
    }
}

fn layout_inlines(
    inlines: &[Inline],
    lay: &mut Lay,
    first: &[Seg],
    hang: &[Seg],
    width: usize,
    style: StyleId,
    code: bool,
) {
    let atoms = flatten(inlines, lay, style);
    if atoms.is_empty() {
        lay.push(first.to_vec(), code);
        return;
    }
    let lines = wrap_glys(&to_glys(&atoms), width, first, hang);
    if lines.is_empty() {
        lay.push(first.to_vec(), code);
    } else {
        for line in lines {
            lay.push(line, code);
        }
    }
}

enum Atom {
    Text {
        text: String,
        style: StyleId,
        link: Option<u32>,
    },
    Break,
}

fn flatten(inlines: &[Inline], lay: &mut Lay, style: StyleId) -> Vec<Atom> {
    let mut atoms = Vec::new();
    flatten_into(inlines, lay, style, &mut atoms);
    atoms
}

fn flatten_into(inlines: &[Inline], lay: &mut Lay, style: StyleId, atoms: &mut Vec<Atom>) {
    for inline in inlines {
        match inline {
            Inline::Text(text) => atoms.push(Atom::Text {
                text: text.clone(),
                style,
                link: None,
            }),
            Inline::Code(text) => atoms.push(Atom::Text {
                text: format!(" {text} "),
                style: StyleId::InlineCode,
                link: None,
            }),
            Inline::Emph(inner) => flatten_into(inner, lay, StyleId::Emph, atoms),
            Inline::Strong(inner) => flatten_into(inner, lay, StyleId::Strong, atoms),
            Inline::Strike(inner) => flatten_into(inner, lay, StyleId::Strike, atoms),
            Inline::Soft => atoms.push(Atom::Text {
                text: " ".to_string(),
                style,
                link: None,
            }),
            Inline::Hard => atoms.push(Atom::Break),
            Inline::Link { text, url } => {
                let label = inline_plain(text);
                let id = lay.alloc_link(url, &label);
                let before = atoms.len();
                flatten_into(text, lay, StyleId::Link, atoms);
                stamp_link(atoms, before, id);
                if is_external(url) {
                    atoms.push(Atom::Text {
                        text: "\u{2060}\u{2197}".to_string(),
                        style: StyleId::Link,
                        link: Some(id),
                    });
                }
            }
            Inline::Image { alt, url } => {
                let label = {
                    let alt = inline_plain(alt);
                    if alt.trim().is_empty() {
                        "image".to_string()
                    } else {
                        format!("image: {alt}")
                    }
                };
                let id = lay.alloc_link(url, &label);
                atoms.push(Atom::Text {
                    text: label,
                    style: StyleId::Link,
                    link: Some(id),
                });
            }
            Inline::Footnote(label) => {
                let anchor = format!("#fn-{}", slugify(label));
                let text = format!("[{label}]");
                let id = lay.alloc_link(&anchor, &text);
                atoms.push(Atom::Text {
                    text,
                    style: StyleId::Link,
                    link: Some(id),
                });
            }
        }
    }
}

fn stamp_link(atoms: &mut [Atom], from: usize, id: u32) {
    for atom in &mut atoms[from..] {
        if let Atom::Text { link, .. } = atom {
            *link = Some(id);
        }
    }
}

fn is_external(url: &str) -> bool {
    let url = url.trim();
    url.starts_with("http://") || url.starts_with("https://") || url.starts_with("mailto:")
}

struct Gly {
    text: String,
    width: usize,
    style: StyleId,
    link: Option<u32>,
    space: bool,
    hard: bool,
}

fn to_glys(atoms: &[Atom]) -> Vec<Gly> {
    let mut glys = Vec::new();
    for atom in atoms {
        match atom {
            Atom::Break => glys.push(Gly {
                text: String::new(),
                width: 0,
                style: StyleId::Text,
                link: None,
                space: false,
                hard: true,
            }),
            Atom::Text { text, style, link } => {
                for grapheme in text.graphemes(true) {
                    let space = grapheme.chars().all(char::is_whitespace);
                    glys.push(Gly {
                        text: grapheme.to_string(),
                        width: UnicodeWidthStr::width(grapheme),
                        style: *style,
                        link: *link,
                        space,
                        hard: false,
                    });
                }
            }
        }
    }
    glys
}

fn wrap_glys(glys: &[Gly], width: usize, first: &[Seg], hang: &[Seg]) -> Vec<Vec<Seg>> {
    let width = width.max(1);
    if glys.is_empty() {
        return vec![first.to_vec()];
    }
    let mut lines = Vec::new();
    let mut cursor = 0;
    let mut line_no = 0;
    while cursor < glys.len() {
        let pre = if line_no == 0 { first } else { hang };
        if line_no > 0 {
            while cursor < glys.len() && glys[cursor].space && !glys[cursor].hard {
                cursor += 1;
            }
        }
        if cursor >= glys.len() {
            break;
        }
        if glys[cursor].hard {
            lines.push(pre.to_vec());
            cursor += 1;
            line_no += 1;
            continue;
        }
        let pre_w = segs_width(pre);
        let budget = width.max(pre_w + 1);
        let start = cursor;
        let mut end = cursor;
        let mut col = pre_w;
        let mut last_break: Option<usize> = None;
        while end < glys.len() && !glys[end].hard {
            let w = glys[end].width;
            if col.saturating_add(w) > budget && end > start {
                break;
            }
            if glys[end].space {
                last_break = Some(end);
            }
            col = col.saturating_add(w);
            end += 1;
            if col >= budget {
                break;
            }
            if w > budget && end == start + 1 {
                break;
            }
        }
        let hard = end < glys.len() && glys[end].hard;
        let mut keep_end = end;
        let mut next = if hard { end + 1 } else { end };
        if !hard && end < glys.len() {
            if let Some(brk) = last_break {
                if brk > start {
                    keep_end = brk;
                    next = brk + 1;
                }
            }
        }
        if next <= start {
            keep_end = (start + 1).min(glys.len());
            next = keep_end;
        }
        if keep_end < start {
            keep_end = start;
        }
        let mut segs = pre.to_vec();
        segs.extend(glys_to_segs(&glys[start..keep_end]));
        lines.push(segs);
        cursor = next;
        line_no += 1;
        if line_no > glys.len() + 2 {
            break;
        }
    }
    if lines.is_empty() {
        lines.push(first.to_vec());
    }
    lines
}

fn glys_to_segs(glys: &[Gly]) -> Vec<Seg> {
    let mut segs: Vec<Seg> = Vec::new();
    for gly in glys {
        if gly.hard {
            continue;
        }
        if let Some(last) = segs.last_mut() {
            if last.style == gly.style && last.link == gly.link {
                last.text.push_str(&gly.text);
                continue;
            }
        }
        segs.push(Seg::linked(gly.text.clone(), gly.style, gly.link));
    }
    segs
}

fn segs_width(segs: &[Seg]) -> usize {
    segs.iter()
        .map(|seg| UnicodeWidthStr::width(seg.text.as_str()))
        .sum()
}

fn quote_prefix(prefix: &[Seg]) -> bool {
    prefix.iter().any(|seg| seg.style == StyleId::QuoteBar)
}

fn digits(n: usize) -> usize {
    let mut d = 1;
    let mut v = 10usize;
    while n >= v {
        d += 1;
        if v > usize::MAX / 10 {
            break;
        }
        v *= 10;
    }
    d.max(2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    #[test]
    fn wraps_words_and_wide_chars() {
        let doc = parse("hello world\n\n\u{4f60}\u{597d}\u{4e16}\u{754c}\n");
        let rendered = render(&doc.blocks, 8);
        let text = rendered
            .lines
            .iter()
            .map(|line| line.plain.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("hello"));
        assert!(text.contains("world"));
        assert!(!rendered
            .lines
            .iter()
            .any(|line| line.width() > 8 && !line.code));
    }

    #[test]
    fn links_are_tracked_and_fences_hidden() {
        let doc =
            parse("# Title\n\nSee [docs](https://ratatui.rs) now.\n\n```rust\nfn main() {}\n```\n");
        let rendered = render(&doc.blocks, 40);
        assert_eq!(rendered.headings[0].slug, "title");
        assert!(rendered
            .links
            .iter()
            .any(|link| link.url == "https://ratatui.rs"));
        let joined = rendered
            .lines
            .iter()
            .map(|line| line.plain.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!joined.contains("```"));
        assert!(!joined.contains("# Title"));
        assert!(joined.contains("fn"));
        assert!(joined.contains('\u{2197}'));
    }

    #[test]
    fn table_and_task_list() {
        let doc = parse("- [x] done\n\n| Left | Right |\n| :--- | ---: |\n| a | 10 |\n");
        let rendered = render(&doc.blocks, 40);
        let joined = rendered
            .lines
            .iter()
            .map(|l| l.plain.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains('\u{2713}'), "{joined}");
        assert!(joined.contains("done"), "{joined}");
        assert!(joined.contains("Left"), "{joined}");
        assert!(joined.contains("Right"), "{joined}");
        assert!(joined.contains('\u{2500}'), "{joined}");
    }

    #[test]
    fn heading_scale_matches_vscode_rules() {
        let doc = parse("# One\n\n## Two\n\n### Three\n\n#### Four\n\n##### Five\n\n###### Six\n");
        let rendered = render(&doc.blocks, 40);
        let lines: Vec<&str> = rendered
            .lines
            .iter()
            .map(|line| line.plain.as_str())
            .collect();
        let at = |label: &str| lines.iter().position(|line| *line == label).unwrap();
        let one = at("One");
        let two = at("Two");
        let three = at("Three");
        let four = at("Four");
        assert!(lines[one + 1].chars().all(|ch| ch == '\u{2501}'));
        assert_eq!(lines[one + 1].chars().count(), 40);
        assert!(lines[two + 1].chars().all(|ch| ch == '\u{2500}'));
        assert_eq!(lines[two + 1].chars().count(), 40);
        assert!(lines[three + 1].chars().all(|ch| ch == '\u{2500}'));
        assert_eq!(lines[three + 1].chars().count(), "Three".chars().count());
        assert!(lines[three + 1].chars().count() < lines[two + 1].chars().count());
        assert_ne!(lines.get(four + 1).copied().unwrap_or(""), lines[four]);
        assert!(lines[four + 1].trim().is_empty() || !lines[four + 1].chars().all(|ch| ch == '\u{2500}'));
        assert_eq!(rendered.headings[2].level, 3);
        assert_eq!(rendered.headings[3].level, 4);
        assert_eq!(rendered.headings[4].level, 5);
        assert_eq!(rendered.headings[5].level, 6);
        let style_of = |label: &str| rendered.lines[at(label)].spans[0].style;
        assert_eq!(style_of("One"), StyleId::H1);
        assert_eq!(style_of("Two"), StyleId::H2);
        assert_eq!(style_of("Three"), StyleId::H3);
        assert_eq!(style_of("Four"), StyleId::H4);
        assert_eq!(style_of("Five"), StyleId::H5);
        assert_eq!(style_of("Six"), StyleId::H6);
    }
}
