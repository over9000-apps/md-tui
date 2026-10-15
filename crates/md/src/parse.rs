//! CommonMark + GFM block tree. Pulldown-cmark events are consumed once and
//! turned into owned blocks so layout can run again on resize without reparsing.

use pulldown_cmark::{
    Alignment, BlockQuoteKind, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Doc {
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Heading {
        level: u8,
        inlines: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    Code {
        lang: Option<String>,
        code: String,
    },
    Quote {
        kind: Option<QuoteKind>,
        blocks: Vec<Block>,
    },
    List {
        ordered: bool,
        start: u64,
        items: Vec<ListItem>,
    },
    Table {
        alignments: Vec<Align>,
        header: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    Rule,
    Html(String),
    Footnote {
        label: String,
        blocks: Vec<Block>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuoteKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl QuoteKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Note => "Note",
            Self::Tip => "Tip",
            Self::Important => "Important",
            Self::Warning => "Warning",
            Self::Caution => "Caution",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ListItem {
    pub checked: Option<bool>,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Inline {
    Text(String),
    Code(String),
    Emph(Vec<Inline>),
    Strong(Vec<Inline>),
    Strike(Vec<Inline>),
    Link { text: Vec<Inline>, url: String },
    Image { alt: Vec<Inline>, url: String },
    Footnote(String),
    Soft,
    Hard,
}

pub fn parse(markdown: &str) -> Doc {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_GFM);
    let parser = Parser::new_ext(markdown, options);
    let mut iter = parser.peekable();
    let mut blocks = parse_blocks(&mut iter);
    hoist_footnotes(&mut blocks);
    Doc { blocks }
}

fn hoist_footnotes(blocks: &mut Vec<Block>) {
    let mut notes = Vec::new();
    blocks.retain(|b| {
        if matches!(b, Block::Footnote { .. }) {
            notes.push(b.clone());
            false
        } else {
            true
        }
    });
    if notes.is_empty() {
        return;
    }
    blocks.push(Block::Rule);
    blocks.push(Block::Heading {
        level: 2,
        inlines: vec![Inline::Text("Footnotes".to_string())],
    });
    blocks.extend(notes);
}

fn parse_blocks<'a, I>(it: &mut std::iter::Peekable<I>) -> Vec<Block>
where
    I: Iterator<Item = Event<'a>>,
{
    let mut blocks = Vec::new();
    loop {
        match it.next() {
            None => break,
            Some(Event::End(_)) => break,
            Some(Event::Start(tag)) => {
                let mut ignored = None;
                if let Some(block) = parse_block(it, tag, &mut ignored) {
                    blocks.push(block);
                }
            }
            Some(Event::Rule) => blocks.push(Block::Rule),
            Some(Event::Html(html)) => push_html(&mut blocks, html.as_ref()),
            Some(Event::Text(text)) => push_html(&mut blocks, text.as_ref()),
            Some(_) => {}
        }
    }
    blocks
}

fn push_html(blocks: &mut Vec<Block>, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    if let Some(Block::Html(existing)) = blocks.last_mut() {
        existing.push_str(text);
    } else {
        blocks.push(Block::Html(text.to_string()));
    }
}

fn parse_block<'a, I>(
    it: &mut std::iter::Peekable<I>,
    tag: Tag<'a>,
    task: &mut Option<bool>,
) -> Option<Block>
where
    I: Iterator<Item = Event<'a>>,
{
    match tag {
        Tag::Paragraph => {
            let inlines = parse_inlines(it, task);
            if inlines.is_empty() {
                None
            } else {
                Some(Block::Paragraph(inlines))
            }
        }
        Tag::Heading { level, .. } => Some(Block::Heading {
            level: heading_level(level),
            inlines: parse_inlines(it, task),
        }),
        Tag::BlockQuote(kind) => {
            let kind = kind.and_then(quote_kind);
            let mut blocks = parse_blocks(it);
            if kind.is_some() {
                strip_alert_marker(&mut blocks);
            }
            Some(Block::Quote { kind, blocks })
        }
        Tag::CodeBlock(kind) => {
            let lang = code_lang(kind);
            let mut code = String::new();
            loop {
                match it.next() {
                    Some(Event::Text(s)) | Some(Event::Html(s)) | Some(Event::Code(s)) => {
                        code.push_str(s.as_ref());
                    }
                    Some(Event::SoftBreak) | Some(Event::HardBreak) => code.push('\n'),
                    Some(Event::End(_)) => break,
                    Some(Event::Start(_)) => skip_balanced(it),
                    None => break,
                    Some(_) => {}
                }
            }
            if code.ends_with('\n') {
                code.pop();
                if code.ends_with('\r') {
                    code.pop();
                }
            }
            Some(Block::Code { lang, code })
        }
        Tag::List(start) => Some(parse_list(it, start)),
        Tag::Item => Some(Block::List {
            ordered: false,
            start: 1,
            items: vec![parse_item(it)],
        }),
        Tag::HtmlBlock => {
            let mut html = String::new();
            loop {
                match it.next() {
                    Some(Event::Html(s)) | Some(Event::Text(s)) => html.push_str(s.as_ref()),
                    Some(Event::End(_)) => break,
                    Some(Event::Start(_)) => skip_balanced(it),
                    None => break,
                    Some(_) => {}
                }
            }
            if html.trim().is_empty() {
                None
            } else {
                Some(Block::Html(html))
            }
        }
        Tag::FootnoteDefinition(label) => Some(Block::Footnote {
            label: label.to_string(),
            blocks: parse_blocks(it),
        }),
        Tag::Table(alignments) => Some(parse_table(it, alignments)),
        Tag::MetadataBlock(_)
        | Tag::DefinitionList
        | Tag::DefinitionListTitle
        | Tag::DefinitionListDefinition
        | Tag::TableHead
        | Tag::TableRow
        | Tag::TableCell => {
            skip_balanced(it);
            None
        }
        Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Superscript | Tag::Subscript => {
            let inlines = parse_inlines(it, task);
            if inlines.is_empty() {
                None
            } else {
                Some(Block::Paragraph(inlines))
            }
        }
        Tag::Link { dest_url, .. } => {
            let text = parse_inlines(it, task);
            Some(Block::Paragraph(vec![Inline::Link {
                text,
                url: dest_url.to_string(),
            }]))
        }
        Tag::Image { dest_url, .. } => {
            let alt = parse_inlines(it, task);
            Some(Block::Paragraph(vec![Inline::Image {
                alt,
                url: dest_url.to_string(),
            }]))
        }
    }
}

fn parse_list<'a, I>(it: &mut std::iter::Peekable<I>, start: Option<u64>) -> Block
where
    I: Iterator<Item = Event<'a>>,
{
    let mut items = Vec::new();
    loop {
        match it.next() {
            Some(Event::Start(Tag::Item)) => items.push(parse_item(it)),
            Some(Event::End(_)) => break,
            Some(Event::Start(tag)) => {
                let mut ignored = None;
                if let Some(block) = parse_block(it, tag, &mut ignored) {
                    items.push(ListItem {
                        checked: None,
                        blocks: vec![block],
                    });
                }
            }
            None => break,
            Some(_) => {}
        }
    }
    Block::List {
        ordered: start.is_some(),
        start: start.unwrap_or(1),
        items,
    }
}

fn parse_item<'a, I>(it: &mut std::iter::Peekable<I>) -> ListItem
where
    I: Iterator<Item = Event<'a>>,
{
    let mut checked = None;
    let mut blocks = Vec::new();
    loop {
        let step = match it.peek() {
            None => ItemStep::Done,
            Some(Event::End(_)) => ItemStep::End,
            Some(Event::TaskListMarker(_)) => ItemStep::Task,
            Some(Event::Rule) => ItemStep::Rule,
            Some(Event::Start(tag)) if is_block_tag(tag) => ItemStep::Block,
            Some(_) => ItemStep::Inline,
        };
        match step {
            ItemStep::Done => break,
            ItemStep::End => {
                it.next();
                break;
            }
            ItemStep::Task => {
                if let Some(Event::TaskListMarker(value)) = it.next() {
                    checked = Some(value);
                }
            }
            ItemStep::Rule => {
                it.next();
                blocks.push(Block::Rule);
            }
            ItemStep::Block => {
                let Some(Event::Start(tag)) = it.next() else {
                    break;
                };
                if let Some(block) = parse_block(it, tag, &mut checked) {
                    blocks.push(block);
                }
            }
            ItemStep::Inline => {
                let inlines = parse_tight_inlines(it, &mut checked);
                if inlines.is_empty() {
                    it.next();
                } else {
                    blocks.push(Block::Paragraph(inlines));
                }
            }
        }
    }
    ListItem { checked, blocks }
}

enum ItemStep {
    Done,
    End,
    Task,
    Rule,
    Block,
    Inline,
}

fn is_block_tag(tag: &Tag<'_>) -> bool {
    matches!(
        tag,
        Tag::Paragraph
            | Tag::Heading { .. }
            | Tag::BlockQuote(_)
            | Tag::CodeBlock(_)
            | Tag::List(_)
            | Tag::Item
            | Tag::HtmlBlock
            | Tag::FootnoteDefinition(_)
            | Tag::Table(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::MetadataBlock(_)
    )
}

fn parse_tight_inlines<'a, I>(
    it: &mut std::iter::Peekable<I>,
    task: &mut Option<bool>,
) -> Vec<Inline>
where
    I: Iterator<Item = Event<'a>>,
{
    let mut out = Vec::new();
    loop {
        let stop = match it.peek() {
            None | Some(Event::End(_) | Event::Rule | Event::TaskListMarker(_)) => true,
            Some(Event::Start(tag)) if is_block_tag(tag) => true,
            _ => false,
        };
        if stop {
            break;
        }
        match it.next() {
            Some(Event::Text(text)) => push_text(&mut out, text.as_ref()),
            Some(Event::Code(text)) => out.push(Inline::Code(text.to_string())),
            Some(Event::SoftBreak) => out.push(Inline::Soft),
            Some(Event::HardBreak) => out.push(Inline::Hard),
            Some(Event::FootnoteReference(label)) => out.push(Inline::Footnote(label.to_string())),
            Some(Event::Start(Tag::Emphasis)) => out.push(Inline::Emph(parse_inlines(it, task))),
            Some(Event::Start(Tag::Strong)) => out.push(Inline::Strong(parse_inlines(it, task))),
            Some(Event::Start(Tag::Strikethrough)) => {
                out.push(Inline::Strike(parse_inlines(it, task)));
            }
            Some(Event::Start(Tag::Superscript | Tag::Subscript)) => {
                out.extend(parse_inlines(it, task));
            }
            Some(Event::Start(Tag::Link { dest_url, .. })) => out.push(Inline::Link {
                text: parse_inlines(it, task),
                url: dest_url.to_string(),
            }),
            Some(Event::Start(Tag::Image { dest_url, .. })) => out.push(Inline::Image {
                alt: parse_inlines(it, task),
                url: dest_url.to_string(),
            }),
            Some(Event::Html(text) | Event::InlineHtml(text)) => push_text(&mut out, text.as_ref()),
            Some(Event::InlineMath(text) | Event::DisplayMath(text)) => {
                out.push(Inline::Code(text.to_string()));
            }
            Some(Event::Start(_)) => skip_balanced(it),
            Some(_) => {}
            None => break,
        }
    }
    out
}

fn parse_table<'a, I>(it: &mut std::iter::Peekable<I>, alignments: Vec<Alignment>) -> Block
where
    I: Iterator<Item = Event<'a>>,
{
    let alignments = alignments
        .into_iter()
        .map(|a| match a {
            Alignment::Center => Align::Center,
            Alignment::Right => Align::Right,
            Alignment::Left | Alignment::None => Align::Left,
        })
        .collect();
    let mut header = Vec::new();
    let mut rows = Vec::new();
    let mut current: Option<Vec<Vec<Inline>>> = None;
    let mut in_head = false;
    loop {
        match it.next() {
            Some(Event::Start(Tag::TableHead)) => {
                in_head = true;
                current = Some(Vec::new());
            }
            Some(Event::End(TagEnd::TableHead)) => {
                if let Some(row) = current.take() {
                    header = row;
                }
                in_head = false;
            }
            Some(Event::Start(Tag::TableRow)) => current = Some(Vec::new()),
            Some(Event::Start(Tag::TableCell)) => {
                let mut ignored = None;
                let cell = parse_inlines(it, &mut ignored);
                if let Some(row) = current.as_mut() {
                    row.push(cell);
                }
            }
            Some(Event::End(TagEnd::TableRow)) => {
                if let Some(row) = current.take() {
                    if in_head || header.is_empty() {
                        header = row;
                    } else {
                        rows.push(row);
                    }
                }
            }
            Some(Event::End(TagEnd::Table)) => break,
            Some(Event::End(_)) => break,
            Some(Event::Start(_)) => skip_balanced(it),
            None => break,
            Some(_) => {}
        }
    }
    Block::Table {
        alignments,
        header,
        rows,
    }
}

fn parse_inlines<'a, I>(it: &mut std::iter::Peekable<I>, task: &mut Option<bool>) -> Vec<Inline>
where
    I: Iterator<Item = Event<'a>>,
{
    let mut out = Vec::new();
    loop {
        match it.next() {
            None => break,
            Some(Event::End(_)) => break,
            Some(Event::Text(s)) => push_text(&mut out, s.as_ref()),
            Some(Event::Code(s)) => out.push(Inline::Code(s.to_string())),
            Some(Event::SoftBreak) => out.push(Inline::Soft),
            Some(Event::HardBreak) => out.push(Inline::Hard),
            Some(Event::FootnoteReference(label)) => {
                out.push(Inline::Footnote(label.to_string()));
            }
            Some(Event::Start(Tag::Emphasis)) => {
                out.push(Inline::Emph(parse_inlines(it, task)));
            }
            Some(Event::Start(Tag::Strong)) => {
                out.push(Inline::Strong(parse_inlines(it, task)));
            }
            Some(Event::Start(Tag::Strikethrough)) => {
                out.push(Inline::Strike(parse_inlines(it, task)));
            }
            Some(Event::Start(Tag::Superscript | Tag::Subscript)) => {
                out.extend(parse_inlines(it, task));
            }
            Some(Event::Start(Tag::Link { dest_url, .. })) => {
                out.push(Inline::Link {
                    text: parse_inlines(it, task),
                    url: dest_url.to_string(),
                });
            }
            Some(Event::Start(Tag::Image { dest_url, .. })) => {
                out.push(Inline::Image {
                    alt: parse_inlines(it, task),
                    url: dest_url.to_string(),
                });
            }
            Some(Event::Start(_)) => skip_balanced(it),
            Some(Event::Html(s) | Event::InlineHtml(s)) => push_text(&mut out, s.as_ref()),
            Some(Event::TaskListMarker(value)) => *task = Some(value),
            Some(Event::Rule) => {}
            Some(Event::InlineMath(s) | Event::DisplayMath(s)) => {
                out.push(Inline::Code(s.to_string()));
            }
        }
    }
    out
}

fn push_text(out: &mut Vec<Inline>, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(Inline::Text(existing)) = out.last_mut() {
        existing.push_str(text);
    } else {
        out.push(Inline::Text(text.to_string()));
    }
}

fn skip_balanced<'a, I>(it: &mut I)
where
    I: Iterator<Item = Event<'a>>,
{
    let mut depth = 1;
    for event in it.by_ref() {
        match event {
            Event::Start(_) => depth += 1,
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
    }
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn quote_kind(kind: BlockQuoteKind) -> Option<QuoteKind> {
    Some(match kind {
        BlockQuoteKind::Note => QuoteKind::Note,
        BlockQuoteKind::Tip => QuoteKind::Tip,
        BlockQuoteKind::Important => QuoteKind::Important,
        BlockQuoteKind::Warning => QuoteKind::Warning,
        BlockQuoteKind::Caution => QuoteKind::Caution,
    })
}

fn code_lang(kind: CodeBlockKind<'_>) -> Option<String> {
    match kind {
        CodeBlockKind::Fenced(lang) => {
            let token = lang.split_whitespace().next().unwrap_or("").trim();
            if token.is_empty() {
                None
            } else {
                Some(token.to_string())
            }
        }
        CodeBlockKind::Indented => None,
    }
}

fn strip_alert_marker(blocks: &mut Vec<Block>) {
    let is_marker = blocks.first().is_some_and(|block| {
        if let Block::Paragraph(inlines) = block {
            let text = inline_plain(inlines);
            let t = text.trim();
            matches!(
                t.to_ascii_uppercase().as_str(),
                "[!NOTE]" | "[!TIP]" | "[!IMPORTANT]" | "[!WARNING]" | "[!CAUTION]"
            )
        } else {
            false
        }
    });
    if is_marker {
        blocks.remove(0);
    }
}

pub fn inline_plain(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(s) | Inline::Code(s) | Inline::Footnote(s) => out.push_str(s),
            Inline::Emph(inner)
            | Inline::Strong(inner)
            | Inline::Strike(inner)
            | Inline::Link { text: inner, .. }
            | Inline::Image { alt: inner, .. } => out.push_str(&inline_plain(inner)),
            Inline::Soft => out.push(' '),
            Inline::Hard => out.push('\n'),
        }
    }
    out
}

pub fn strip_front_matter(input: &str) -> String {
    let s = input.strip_prefix('\u{feff}').unwrap_or(input);
    for delim in ["---", "+++"] {
        if let Some(rest) = s.strip_prefix(delim) {
            let rest = if let Some(body) = rest.strip_prefix("\r\n") {
                body
            } else if let Some(body) = rest.strip_prefix('\n') {
                body
            } else {
                continue;
            };
            let close = format!("\n{delim}");
            if let Some(idx) = rest.find(&close) {
                let after = &rest[idx + close.len()..];
                let after = after
                    .strip_prefix("\r\n")
                    .or_else(|| after.strip_prefix('\n'))
                    .unwrap_or(after);
                return after.to_string();
            }
        }
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gfm_features() {
        let md = "# Title\n\nA [link](https://example.com) and `code`.\n\n- [x] done\n- [ ] open\n\n~~gone~~\n\n```rust\nfn main() {}\n```\n\n| A | B |\n| --- | ---: |\n| a | 1 |\n\n> [!NOTE]\n> hello\n";
        let doc = parse(md);
        assert!(doc
            .blocks
            .iter()
            .any(|b| matches!(b, Block::Heading { level: 1, .. })));
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::Code { .. })));
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::Table { .. })));
        assert!(doc.blocks.iter().any(|b| matches!(
            b,
            Block::List { items, .. } if items.iter().any(|i| i.checked == Some(true))
        )));
        assert!(doc.blocks.iter().any(|b| matches!(
            b,
            Block::Quote {
                kind: Some(QuoteKind::Note),
                ..
            }
        )));
        let para = doc
            .blocks
            .iter()
            .find_map(|b| match b {
                Block::Paragraph(inlines) => Some(inlines),
                _ => None,
            })
            .unwrap();
        assert!(para
            .iter()
            .any(|i| matches!(i, Inline::Link { url, .. } if url == "https://example.com")));
    }

    #[test]
    fn task_markers_survive_a_loose_list() {
        let md = "- item\n\n- [x] done\n- [ ] open\n";
        let doc = parse(md);
        let Block::List { items, .. } = &doc.blocks[0] else {
            panic!("{doc:?}");
        };
        assert!(
            items.iter().any(|item| item.checked == Some(true)),
            "{items:?}"
        );
        assert!(items.iter().any(|item| item.checked == Some(false)));
    }

    #[test]
    fn strips_yaml_front_matter() {
        let raw = "---\ntitle: hi\n---\n# After\n";
        assert!(strip_front_matter(raw).trim_start().starts_with("# After"));
    }
}
