//! Resolve a Markdown link to an in-app file, an anchor, or an external opener.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Browser(String),
    File {
        path: PathBuf,
        anchor: Option<String>,
    },
    Anchor(String),
    Unsupported(String),
}

pub fn resolve_link(base_file: Option<&Path>, url: &str) -> Target {
    let url = url.trim();
    if url.is_empty() {
        return Target::Unsupported("empty link".into());
    }
    if url.starts_with('#') && !url.contains("://") {
        return Target::Anchor(percent_decode(&url[1..]));
    }
    if is_external_scheme(url) {
        return Target::Browser(url.to_string());
    }

    let bare = url.strip_prefix("file://").unwrap_or(url);
    let (path_str, anchor) = split_anchor(bare);
    let anchor = anchor.map(percent_decode);
    if path_str.is_empty() {
        return match anchor {
            Some(anchor) => Target::Anchor(anchor),
            None => Target::Unsupported("empty link".into()),
        };
    }
    let decoded = percent_decode(path_str);
    let rel = Path::new(&decoded);
    if rel.is_absolute() {
        return Target::File {
            path: rel.to_path_buf(),
            anchor,
        };
    }
    let Some(base) = base_file else {
        return Target::Unsupported("relative link needs a file path (this view is stdin)".into());
    };
    let dir = base
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    Target::File {
        path: dir.join(rel),
        anchor,
    }
}

pub fn is_markdown(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => {
            let ext = ext.to_ascii_lowercase();
            matches!(ext.as_str(), "md" | "markdown" | "mdown" | "mkd")
        }
        None => false,
    }
}

pub fn open_external(target: &str) -> Result<(), String> {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let mut child = Command::new(program)
        .arg(target)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("could not launch {program}: {err}"))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

fn is_external_scheme(url: &str) -> bool {
    let Some(scheme) = url.split_once("://").map(|(scheme, _)| scheme) else {
        return url.starts_with("mailto:");
    };
    if scheme.eq_ignore_ascii_case("file") {
        return false;
    }
    !scheme.is_empty()
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '.' || c == '-')
}

fn split_anchor(url: &str) -> (&str, Option<&str>) {
    match url.split_once('#') {
        Some((path, anchor)) => (path, Some(anchor)),
        None => (url, None),
    }
}

pub fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out)
        .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned())
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub fn slugify(text: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in text.chars() {
        if c == '\'' || c == '’' {
            continue;
        }
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if c.is_alphanumeric() {
            for lower in c.to_lowercase() {
                out.push(lower);
            }
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_http_anchor_and_relative() {
        assert!(matches!(
            resolve_link(None, "https://ratatui.rs"),
            Target::Browser(_)
        ));
        assert_eq!(
            resolve_link(None, "#Tables"),
            Target::Anchor("Tables".into())
        );
        let base = Path::new("/docs/guide.md");
        match resolve_link(Some(base), "./notes.md#Intro") {
            Target::File { path, anchor } => {
                assert_eq!(path, PathBuf::from("/docs/notes.md"));
                assert_eq!(anchor.as_deref(), Some("Intro"));
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(matches!(
            resolve_link(None, "notes.md"),
            Target::Unsupported(_)
        ));
    }

    #[test]
    fn decodes_and_slugs() {
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(slugify("Why this exists"), "why-this-exists");
        assert_eq!(slugify("What's new"), "whats-new");
    }
}
