//! Small line-oriented highlighter. It covers common languages well enough to
//! make fenced blocks scannable without pulling in syntect.

use crate::theme::StyleId;

#[derive(Clone, Debug, Default)]
pub struct HlState {
    pub block_comment: bool,
}

pub fn highlight_line(lang: &str, line: &str, state: &mut HlState) -> Vec<(String, StyleId)> {
    let owned = canonical_lang(lang);
    let lang = owned.as_str();
    let spec = spec_for(lang);
    if spec.is_none() && !state.block_comment {
        return vec![(line.to_string(), StyleId::Code)];
    }
    let spec = spec.unwrap_or(Spec {
        line_comment: None,
        block: false,
        hash: false,
        html: false,
        keywords: &[],
        types: &[],
    });

    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let mut out: Vec<(String, StyleId)> = Vec::new();
    let mut buf = String::new();
    let mut buf_style = StyleId::Code;

    let flush = |buf: &mut String, style: &mut StyleId, out: &mut Vec<(String, StyleId)>| {
        if !buf.is_empty() {
            out.push((std::mem::take(buf), *style));
        }
    };
    let push = |buf: &mut String,
                buf_style: &mut StyleId,
                out: &mut Vec<(String, StyleId)>,
                ch: char,
                style: StyleId| {
        if *buf_style != style && !buf.is_empty() {
            out.push((std::mem::take(buf), *buf_style));
        }
        *buf_style = style;
        buf.push(ch);
    };

    if state.block_comment {
        while i < chars.len() {
            buf.push(chars[i]);
            if spec.block && chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                buf.push('/');
                i += 2;
                state.block_comment = false;
                break;
            }
            if spec.html && chars[i] == '-' && chars.get(i + 1..i + 3) == Some(&['-', '>']) {
                buf.push('-');
                buf.push('>');
                i += 3;
                state.block_comment = false;
                break;
            }
            i += 1;
        }
        flush(&mut buf, &mut buf_style, &mut out);
        buf_style = StyleId::Code;
        if state.block_comment {
            if out.is_empty() {
                out.push((String::new(), StyleId::CodeComment));
            } else if let Some(last) = out.last_mut() {
                last.1 = StyleId::CodeComment;
            }
            return out;
        }
        if let Some(last) = out.last_mut() {
            last.1 = StyleId::CodeComment;
        }
    }

    while i < chars.len() {
        let c = chars[i];
        if spec.html && starts_with(&chars[i..], "<!--") {
            flush(&mut buf, &mut buf_style, &mut out);
            let rest: String = chars[i..].iter().collect();
            if let Some(end) = rest.find("-->") {
                out.push((rest[..=end + 2].to_string(), StyleId::CodeComment));
                i += end + 3;
            } else {
                out.push((rest, StyleId::CodeComment));
                state.block_comment = true;
                break;
            }
            continue;
        }
        if spec.block && c == '/' && chars.get(i + 1) == Some(&'*') {
            flush(&mut buf, &mut buf_style, &mut out);
            let start = i;
            i += 2;
            let mut ended = false;
            while i < chars.len() {
                if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    i += 2;
                    ended = true;
                    break;
                }
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            out.push((text, StyleId::CodeComment));
            if !ended {
                state.block_comment = true;
            }
            continue;
        }
        if let Some(marker) = spec.line_comment {
            if starts_with(&chars[i..], marker) {
                flush(&mut buf, &mut buf_style, &mut out);
                let text: String = chars[i..].iter().collect();
                out.push((text, StyleId::CodeComment));
                break;
            }
        }
        if spec.hash && c == '#' {
            flush(&mut buf, &mut buf_style, &mut out);
            let text: String = chars[i..].iter().collect();
            out.push((text, StyleId::CodeComment));
            break;
        }
        if c == '"' || c == '\'' || c == '`' {
            flush(&mut buf, &mut buf_style, &mut out);
            let quote = c;
            let start = i;
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 2;
                    continue;
                }
                if chars[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            let style = if lang == "json" && next_non_ws(&chars, i) == Some(':') {
                StyleId::CodeType
            } else {
                StyleId::CodeString
            };
            out.push((text, style));
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit()))
        {
            flush(&mut buf, &mut buf_style, &mut out);
            let start = i;
            i += 1;
            while i < chars.len()
                && (chars[i].is_ascii_hexdigit() || matches!(chars[i], '.' | '_' | 'x' | 'o' | 'b'))
            {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            out.push((text, StyleId::CodeNumber));
            continue;
        }
        if is_ident_start(c) {
            flush(&mut buf, &mut buf_style, &mut out);
            let start = i;
            i += 1;
            while i < chars.len() && is_ident_continue(chars[i]) {
                i += 1;
            }
            if lang == "rust" && chars.get(i) == Some(&'!') {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            let style = classify_ident(&text, &spec, next_non_ws(&chars, i) == Some('('));
            out.push((text, style));
            continue;
        }
        if c.is_whitespace() {
            push(&mut buf, &mut buf_style, &mut out, c, StyleId::Code);
            i += 1;
            continue;
        }
        push(&mut buf, &mut buf_style, &mut out, c, StyleId::CodePunct);
        i += 1;
    }
    flush(&mut buf, &mut buf_style, &mut out);
    if out.is_empty() {
        out.push((String::new(), StyleId::Code));
    }
    out
}

struct Spec {
    line_comment: Option<&'static str>,
    block: bool,
    hash: bool,
    html: bool,
    keywords: &'static [&'static str],
    types: &'static [&'static str],
}

fn spec_for(lang: &str) -> Option<Spec> {
    let keywords: &[&str] = match lang {
        "rust" => &[
            "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
            "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
            "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
            "trait", "true", "type", "unsafe", "use", "where", "while",
        ],
        "python" => &[
            "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del",
            "elif", "else", "except", "False", "finally", "for", "from", "global", "if", "import",
            "in", "is", "lambda", "None", "nonlocal", "not", "or", "pass", "raise", "return",
            "True", "try", "while", "with", "yield",
        ],
        "javascript" | "typescript" => &[
            "async",
            "await",
            "break",
            "case",
            "catch",
            "class",
            "const",
            "continue",
            "debugger",
            "default",
            "delete",
            "do",
            "else",
            "export",
            "extends",
            "false",
            "finally",
            "for",
            "from",
            "function",
            "if",
            "import",
            "in",
            "instanceof",
            "let",
            "new",
            "null",
            "return",
            "static",
            "super",
            "switch",
            "this",
            "throw",
            "true",
            "try",
            "typeof",
            "undefined",
            "var",
            "void",
            "while",
            "yield",
        ],
        "go" => &[
            "break",
            "case",
            "chan",
            "const",
            "continue",
            "default",
            "defer",
            "else",
            "fallthrough",
            "for",
            "func",
            "go",
            "goto",
            "if",
            "import",
            "interface",
            "map",
            "package",
            "range",
            "return",
            "select",
            "struct",
            "switch",
            "type",
            "var",
        ],
        "c" | "cpp" => &[
            "auto",
            "break",
            "case",
            "char",
            "const",
            "continue",
            "default",
            "do",
            "double",
            "else",
            "enum",
            "extern",
            "float",
            "for",
            "goto",
            "if",
            "inline",
            "int",
            "long",
            "register",
            "return",
            "short",
            "signed",
            "sizeof",
            "static",
            "struct",
            "switch",
            "typedef",
            "union",
            "unsigned",
            "void",
            "volatile",
            "while",
            "class",
            "namespace",
            "template",
            "typename",
            "public",
            "private",
            "protected",
            "virtual",
            "new",
            "delete",
            "bool",
            "true",
            "false",
            "nullptr",
        ],
        "bash" => &[
            "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac", "in",
            "function", "return", "local", "export", "echo",
        ],
        "sql" => &[
            "select", "from", "where", "and", "or", "insert", "into", "values", "update", "set",
            "delete", "create", "table", "drop", "join", "left", "right", "inner", "on", "group",
            "by", "order", "limit", "as", "not", "null", "primary", "key",
        ],
        "html" | "xml" => &[],
        "css" => &[],
        "json" | "yaml" | "toml" | "markdown" | "text" => &[],
        _ => return None,
    };
    let types: &[&str] = match lang {
        "rust" => &[
            "bool", "char", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64",
            "u128", "usize", "f32", "f64", "str", "String", "Vec", "Option", "Result",
        ],
        "typescript" | "javascript" => &[
            "string", "number", "boolean", "any", "never", "unknown", "void", "object",
        ],
        "go" => &[
            "bool",
            "byte",
            "complex64",
            "complex128",
            "error",
            "float32",
            "float64",
            "int",
            "int8",
            "int16",
            "int32",
            "int64",
            "rune",
            "string",
            "uint",
            "uintptr",
        ],
        "python" => &[
            "int", "str", "float", "bool", "list", "dict", "set", "tuple",
        ],
        _ => &[],
    };
    let (line_comment, block, hash, html) = match lang {
        "python" | "bash" | "yaml" | "toml" | "ruby" => (None, false, true, false),
        "sql" => (Some("--"), true, false, false),
        "html" | "xml" | "markdown" => (None, false, false, true),
        "css" => (None, true, false, false),
        "json" | "text" => (None, false, false, false),
        _ => (Some("//"), true, false, false),
    };
    Some(Spec {
        line_comment,
        block,
        hash,
        html,
        keywords,
        types,
    })
}

pub fn canonical_lang(lang: &str) -> String {
    let token = lang
        .trim()
        .split([' ', ','])
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match token.as_str() {
        "rs" => "rust".to_string(),
        "py" | "python3" => "python".to_string(),
        "js" | "jsx" | "mjs" | "cjs" => "javascript".to_string(),
        "ts" | "tsx" => "typescript".to_string(),
        "sh" | "zsh" | "shell" | "console" => "bash".to_string(),
        "yml" => "yaml".to_string(),
        "c++" | "cc" | "cxx" | "hpp" | "hh" | "h" => "cpp".to_string(),
        "golang" => "go".to_string(),
        "md" => "markdown".to_string(),
        "plaintext" | "txt" => "text".to_string(),
        other => other.to_string(),
    }
}

fn classify_ident(text: &str, spec: &Spec, call: bool) -> StyleId {
    let bare = text.trim_end_matches('!');
    let is_keyword = spec.keywords.iter().any(|k| {
        if k.chars().any(|c| c.is_ascii_uppercase()) {
            *k == bare
        } else {
            k.eq_ignore_ascii_case(bare)
        }
    });
    if is_keyword {
        return StyleId::CodeKeyword;
    }
    if spec.types.iter().any(|k| *k == bare) {
        return StyleId::CodeType;
    }
    if bare.chars().next().is_some_and(|c| c.is_uppercase()) {
        return StyleId::CodeType;
    }
    if call {
        StyleId::CodeFunc
    } else {
        StyleId::Code
    }
}

fn starts_with(chars: &[char], marker: &str) -> bool {
    let m: Vec<char> = marker.chars().collect();
    chars.len() >= m.len() && chars[..m.len()] == m[..]
}

fn next_non_ws(chars: &[char], mut i: usize) -> Option<char> {
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    chars.get(i).copied()
}

fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic()
}

fn is_ident_continue(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_rust_keywords_and_strings() {
        let mut state = HlState::default();
        let toks = highlight_line("rust", "fn main() { let s = \"hi\"; }", &mut state);
        assert!(toks
            .iter()
            .any(|(t, s)| t == "fn" && *s == StyleId::CodeKeyword));
        assert!(toks
            .iter()
            .any(|(t, s)| t == "\"hi\"" && *s == StyleId::CodeString));
    }
}
