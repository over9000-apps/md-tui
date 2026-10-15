//! `md` — a full-screen Markdown reader.
//!
//! Pipeline: parse (pulldown-cmark) → layout (wrap + link map) → render
//! (ratatui) → input (keys, mouse, search, history).

mod app;
mod highlight;
mod layout;
mod open;
mod parse;
mod theme;
mod ui;

use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{CommandFactory, Parser, ValueEnum};

use crate::app::{read_path, read_stdin, App};
use crate::layout::render;
use crate::parse::{parse, strip_front_matter};
use crate::theme::{Theme, ThemeName};
use crate::ui::to_ansi;

#[derive(Parser, Debug)]
#[command(
    name = "md",
    version,
    about = "Read Markdown in a full-screen terminal view",
    after_help = "\
Examples:
  md README.md
  md --theme paper NOTES.md
  cat README.md | md
  md --print README.md | less -R

Press ? inside the viewer for the full key list."
)]
struct Cli {
    /// Markdown file to open. Use - to read stdin.
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,

    /// Color theme: ink (dark), paper (light), or terminal (host colors).
    #[arg(short, long, value_enum, default_value_t = ThemeChoice::Ink)]
    theme: ThemeChoice,

    /// Print styled text to stdout and exit.
    #[arg(short, long)]
    print: bool,

    /// Wrap width for --print. Defaults to the terminal width, or 80.
    #[arg(short = 'w', long)]
    width: Option<usize>,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum ThemeChoice {
    Ink,
    Paper,
    Terminal,
}

impl From<ThemeChoice> for ThemeName {
    fn from(value: ThemeChoice) -> Self {
        match value {
            ThemeChoice::Ink => ThemeName::Ink,
            ThemeChoice::Paper => ThemeName::Paper,
            ThemeChoice::Terminal => ThemeName::Terminal,
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("md: {err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    if cli.file.is_none() && io::stdin().is_terminal() {
        let mut cmd = Cli::command();
        let _ = cmd.print_help();
        eprintln!("\nerror: no file given and stdin is a terminal");
        std::process::exit(2);
    }

    let loaded = load(&cli)?;
    let theme_name = ThemeName::from(cli.theme);
    if cli.print || !io::stdout().is_terminal() {
        let width = cli.width.unwrap_or_else(terminal_width);
        let doc = parse(&loaded.text);
        let rendered = render(&doc.blocks, width.max(1));
        print!("{}", to_ansi(&rendered, &Theme::get(theme_name)));
        return Ok(());
    }

    let app = App::new(
        loaded.title,
        loaded.path,
        loaded.text,
        loaded.lossy,
        theme_name,
    );
    ui::run(app)
}

struct Loaded {
    title: String,
    path: Option<PathBuf>,
    text: String,
    lossy: bool,
}

fn load(cli: &Cli) -> Result<Loaded, String> {
    match &cli.file {
        Some(path) if path.as_os_str() == "-" => stdin_loaded(),
        Some(path) => {
            let (text, lossy) = read_path(path)?;
            Ok(Loaded {
                title: path.display().to_string(),
                path: Some(path.clone()),
                text: strip_front_matter(&text),
                lossy,
            })
        }
        None => stdin_loaded(),
    }
}

fn stdin_loaded() -> Result<Loaded, String> {
    let (text, lossy) = read_stdin()?;
    Ok(Loaded {
        title: "stdin".into(),
        path: None,
        text: strip_front_matter(&text),
        lossy,
    })
}

fn terminal_width() -> usize {
    crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80)
        .max(20)
}
