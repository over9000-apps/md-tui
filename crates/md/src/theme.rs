//! Color themes. Semantic styles are mapped to faces at draw time so switching
//! themes does not rebuild the layout.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeName {
    Ink,
    Paper,
    Terminal,
}

impl ThemeName {
    pub fn cycle(self) -> Self {
        match self {
            Self::Ink => Self::Paper,
            Self::Paper => Self::Terminal,
            Self::Terminal => Self::Ink,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Ink => "ink",
            Self::Paper => "paper",
            Self::Terminal => "terminal",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleId {
    Text,
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
    Emph,
    Strong,
    Strike,
    Link,
    InlineCode,
    Code,
    CodeKeyword,
    CodeString,
    CodeComment,
    CodeNumber,
    CodeType,
    CodeFunc,
    CodePunct,
    Quote,
    QuoteBar,
    Alert,
    ListMarker,
    TaskDone,
    TaskOpen,
    Rule,
    TableHeader,
    TableCell,
    Muted,
    Lang,
    Gutter,
    Html,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paint {
    Rgb(u8, u8, u8),
    Ansi(u8),
    Reset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Face {
    pub fg: Paint,
    pub bg: Option<Paint>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
}

impl Face {
    fn new(fg: Paint) -> Self {
        Self {
            fg,
            bg: None,
            bold: false,
            italic: false,
            underline: false,
            strike: false,
        }
    }

    fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    fn under(mut self) -> Self {
        self.underline = true;
        self
    }

    fn strike(mut self) -> Self {
        self.strike = true;
        self
    }

    fn bg(mut self, bg: Paint) -> Self {
        self.bg = Some(bg);
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub name: ThemeName,
    pub bg: Paint,
    pub text: Paint,
    pub muted: Paint,
    pub h1: Paint,
    pub h2: Paint,
    pub h3: Paint,
    pub h4: Paint,
    pub h5: Paint,
    pub link: Paint,
    pub link_hover_bg: Paint,
    pub focus_bg: Paint,
    pub inline_code_bg: Paint,
    pub inline_code_fg: Paint,
    pub code_bg: Paint,
    pub code_fg: Paint,
    pub keyword: Paint,
    pub string: Paint,
    pub comment: Paint,
    pub number: Paint,
    pub ty: Paint,
    pub func: Paint,
    pub punct: Paint,
    pub quote: Paint,
    pub quote_bar: Paint,
    pub alert: Paint,
    pub rule: Paint,
    pub task_done: Paint,
    pub task_open: Paint,
    pub marker: Paint,
    pub lang: Paint,
    pub gutter: Paint,
    pub status_bg: Paint,
    pub status_fg: Paint,
    pub status_accent: Paint,
    pub search_bg: Paint,
    pub search_fg: Paint,
    pub search_current_bg: Paint,
    pub outline_bg: Paint,
    pub outline_sel_bg: Paint,
    pub popup_bg: Paint,
    pub popup_fg: Paint,
    pub popup_border: Paint,
    pub fill_bg: bool,
}

impl Theme {
    pub fn get(name: ThemeName) -> Self {
        match name {
            ThemeName::Ink => Self::ink(),
            ThemeName::Paper => Self::paper(),
            ThemeName::Terminal => Self::terminal(),
        }
    }

    pub fn face(self, id: StyleId) -> Face {
        let t = self.text;
        match id {
            StyleId::Text => Face::new(t),
            // VS Code markdown.css: h1 2em, h2 1.5em, h3 1.25em, h4 1em,
            // h5 0.875em, h6 0.85em, all weight 600. A cell grid cannot change
            // glyph size, so weight and rule treatment carry the scale:
            // h1–h3 stay bold, h4 is bold at body color, h5–h6 step down.
            StyleId::H1 => Face::new(self.h1).bold(),
            StyleId::H2 => Face::new(self.h2).bold(),
            StyleId::H3 => Face::new(self.h3).bold(),
            StyleId::H4 => Face::new(self.h4).bold(),
            StyleId::H5 => Face::new(self.h5),
            StyleId::H6 => Face::new(self.muted),
            StyleId::Emph => Face::new(t).italic(),
            StyleId::Strong => Face::new(t).bold(),
            StyleId::Strike => Face::new(self.muted).strike(),
            StyleId::Link => Face::new(self.link).under(),
            StyleId::InlineCode => Face::new(self.inline_code_fg).bg(self.inline_code_bg),
            StyleId::Code => Face::new(self.code_fg),
            StyleId::CodeKeyword => Face::new(self.keyword).bold(),
            StyleId::CodeString => Face::new(self.string),
            StyleId::CodeComment => Face::new(self.comment).italic(),
            StyleId::CodeNumber => Face::new(self.number),
            StyleId::CodeType => Face::new(self.ty),
            StyleId::CodeFunc => Face::new(self.func),
            StyleId::CodePunct => Face::new(self.punct),
            StyleId::Quote => Face::new(self.quote).italic(),
            StyleId::QuoteBar => Face::new(self.quote_bar).bold(),
            StyleId::Alert => Face::new(self.alert).bold(),
            StyleId::ListMarker => Face::new(self.marker).bold(),
            StyleId::TaskDone => Face::new(self.task_done).bold(),
            StyleId::TaskOpen => Face::new(self.task_open),
            StyleId::Rule => Face::new(self.rule),
            StyleId::TableHeader => Face::new(self.h3).bold(),
            StyleId::TableCell => Face::new(t),
            StyleId::Muted => Face::new(self.muted),
            StyleId::Lang => Face::new(self.lang).bold(),
            StyleId::Gutter => Face::new(self.gutter),
            StyleId::Html => Face::new(self.muted).italic(),
        }
    }

    fn ink() -> Self {
        Self {
            name: ThemeName::Ink,
            bg: Paint::Rgb(20, 22, 28),
            text: Paint::Rgb(230, 225, 216),
            muted: Paint::Rgb(140, 136, 128),
            h1: Paint::Rgb(242, 206, 150),
            h2: Paint::Rgb(206, 224, 234),
            h3: Paint::Rgb(236, 232, 224),
            h4: Paint::Rgb(230, 225, 216),
            h5: Paint::Rgb(176, 170, 160),
            link: Paint::Rgb(110, 176, 230),
            link_hover_bg: Paint::Rgb(40, 58, 78),
            focus_bg: Paint::Rgb(58, 48, 28),
            inline_code_bg: Paint::Rgb(38, 42, 54),
            inline_code_fg: Paint::Rgb(232, 196, 140),
            code_bg: Paint::Rgb(28, 31, 40),
            code_fg: Paint::Rgb(216, 214, 208),
            keyword: Paint::Rgb(224, 140, 156),
            string: Paint::Rgb(168, 214, 160),
            comment: Paint::Rgb(120, 124, 112),
            number: Paint::Rgb(230, 192, 123),
            ty: Paint::Rgb(186, 176, 230),
            func: Paint::Rgb(130, 190, 220),
            punct: Paint::Rgb(160, 156, 148),
            quote: Paint::Rgb(210, 198, 176),
            quote_bar: Paint::Rgb(212, 165, 116),
            alert: Paint::Rgb(240, 190, 120),
            rule: Paint::Rgb(96, 100, 114),
            task_done: Paint::Rgb(143, 188, 143),
            task_open: Paint::Rgb(160, 156, 148),
            marker: Paint::Rgb(212, 165, 116),
            lang: Paint::Rgb(242, 204, 143),
            gutter: Paint::Rgb(110, 112, 120),
            status_bg: Paint::Rgb(32, 36, 48),
            status_fg: Paint::Rgb(210, 206, 198),
            status_accent: Paint::Rgb(242, 204, 143),
            search_bg: Paint::Rgb(92, 74, 32),
            search_fg: Paint::Rgb(255, 244, 214),
            search_current_bg: Paint::Rgb(176, 120, 40),
            outline_bg: Paint::Rgb(26, 28, 36),
            outline_sel_bg: Paint::Rgb(48, 44, 36),
            popup_bg: Paint::Rgb(28, 30, 38),
            popup_fg: Paint::Rgb(230, 225, 216),
            popup_border: Paint::Rgb(90, 86, 74),
            fill_bg: true,
        }
    }

    fn paper() -> Self {
        Self {
            name: ThemeName::Paper,
            bg: Paint::Rgb(247, 243, 235),
            text: Paint::Rgb(44, 40, 37),
            muted: Paint::Rgb(110, 102, 94),
            h1: Paint::Rgb(122, 52, 42),
            h2: Paint::Rgb(30, 74, 112),
            h3: Paint::Rgb(44, 40, 37),
            h4: Paint::Rgb(44, 40, 37),
            h5: Paint::Rgb(90, 82, 74),
            link: Paint::Rgb(24, 82, 140),
            link_hover_bg: Paint::Rgb(220, 232, 242),
            focus_bg: Paint::Rgb(242, 226, 196),
            inline_code_bg: Paint::Rgb(236, 228, 214),
            inline_code_fg: Paint::Rgb(120, 56, 28),
            code_bg: Paint::Rgb(239, 234, 224),
            code_fg: Paint::Rgb(40, 38, 36),
            keyword: Paint::Rgb(156, 48, 72),
            string: Paint::Rgb(32, 110, 64),
            comment: Paint::Rgb(120, 116, 104),
            number: Paint::Rgb(140, 84, 20),
            ty: Paint::Rgb(72, 64, 140),
            func: Paint::Rgb(24, 90, 130),
            punct: Paint::Rgb(90, 84, 78),
            quote: Paint::Rgb(72, 60, 48),
            quote_bar: Paint::Rgb(160, 96, 48),
            alert: Paint::Rgb(140, 72, 20),
            rule: Paint::Rgb(196, 186, 170),
            task_done: Paint::Rgb(36, 120, 72),
            task_open: Paint::Rgb(110, 102, 94),
            marker: Paint::Rgb(160, 96, 48),
            lang: Paint::Rgb(122, 52, 42),
            gutter: Paint::Rgb(150, 142, 130),
            status_bg: Paint::Rgb(232, 224, 210),
            status_fg: Paint::Rgb(44, 40, 37),
            status_accent: Paint::Rgb(122, 52, 42),
            search_bg: Paint::Rgb(240, 210, 140),
            search_fg: Paint::Rgb(44, 32, 12),
            search_current_bg: Paint::Rgb(230, 160, 60),
            outline_bg: Paint::Rgb(240, 234, 224),
            outline_sel_bg: Paint::Rgb(226, 214, 190),
            popup_bg: Paint::Rgb(252, 249, 242),
            popup_fg: Paint::Rgb(44, 40, 37),
            popup_border: Paint::Rgb(180, 168, 150),
            fill_bg: true,
        }
    }

    fn terminal() -> Self {
        // Indexed ANSI only, and no forced background, so the host palette shows through.
        Self {
            name: ThemeName::Terminal,
            bg: Paint::Reset,
            text: Paint::Reset,
            muted: Paint::Ansi(8),
            h1: Paint::Ansi(11),
            h2: Paint::Ansi(14),
            h3: Paint::Ansi(15),
            h4: Paint::Reset,
            h5: Paint::Ansi(7),
            link: Paint::Ansi(12),
            link_hover_bg: Paint::Ansi(8),
            focus_bg: Paint::Ansi(3),
            inline_code_bg: Paint::Reset,
            inline_code_fg: Paint::Ansi(11),
            code_bg: Paint::Reset,
            code_fg: Paint::Reset,
            keyword: Paint::Ansi(13),
            string: Paint::Ansi(10),
            comment: Paint::Ansi(8),
            number: Paint::Ansi(11),
            ty: Paint::Ansi(14),
            func: Paint::Ansi(12),
            punct: Paint::Ansi(7),
            quote: Paint::Ansi(7),
            quote_bar: Paint::Ansi(11),
            alert: Paint::Ansi(11),
            rule: Paint::Ansi(8),
            task_done: Paint::Ansi(10),
            task_open: Paint::Ansi(8),
            marker: Paint::Ansi(11),
            lang: Paint::Ansi(14),
            gutter: Paint::Ansi(8),
            status_bg: Paint::Ansi(0),
            status_fg: Paint::Ansi(15),
            status_accent: Paint::Ansi(11),
            search_bg: Paint::Ansi(3),
            search_fg: Paint::Ansi(0),
            search_current_bg: Paint::Ansi(11),
            outline_bg: Paint::Reset,
            outline_sel_bg: Paint::Ansi(8),
            popup_bg: Paint::Reset,
            popup_fg: Paint::Reset,
            popup_border: Paint::Ansi(8),
            fill_bg: false,
        }
    }
}
