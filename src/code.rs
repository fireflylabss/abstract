//! Minimal syntax highlighting for fenced code blocks: a hand-written
//! scanner over keywords, strings, comments and numbers — no grammars.
//! Pure — no gpui — so it is testable headless.

use std::ops::Range;

use crate::md::{COMMENT, KEYWORD, NUMBER, STRING};

const RUST: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while",
];
const JS: &[&str] = &[
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
    "of",
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
    "with",
    "yield",
    "interface",
    "type",
    "enum",
    "implements",
    "private",
    "protected",
    "public",
    "readonly",
    "abstract",
    "namespace",
    "declare",
    "satisfies",
    "keyof",
];
const PY: &[&str] = &[
    "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif",
    "else", "except", "False", "finally", "for", "from", "global", "if", "import", "in", "is",
    "lambda", "None", "nonlocal", "not", "or", "pass", "raise", "return", "True", "try", "while",
    "with", "yield",
];
const GO: &[&str] = &[
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
    "true",
    "false",
    "nil",
    "iota",
];
const C: &[&str] = &[
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
    "restrict",
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
    "bool",
    "true",
    "false",
    "class",
    "namespace",
    "template",
    "typename",
    "using",
    "virtual",
    "public",
    "private",
    "protected",
    "new",
    "delete",
    "this",
    "try",
    "catch",
    "throw",
    "constexpr",
    "nullptr",
    "auto",
];
const JAVA: &[&str] = &[
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "final",
    "finally",
    "float",
    "for",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "try",
    "void",
    "volatile",
    "while",
    "true",
    "false",
    "null",
    "var",
    "record",
    "sealed",
];
const KOTLIN: &[&str] = &[
    "as",
    "break",
    "class",
    "continue",
    "do",
    "else",
    "false",
    "for",
    "fun",
    "if",
    "in",
    "interface",
    "is",
    "null",
    "object",
    "package",
    "return",
    "super",
    "this",
    "throw",
    "true",
    "try",
    "typealias",
    "typeof",
    "val",
    "var",
    "when",
    "while",
    "data",
    "sealed",
    "companion",
    "init",
    "suspend",
    "override",
    "private",
    "protected",
    "public",
    "internal",
    "abstract",
    "open",
    "enum",
];
const SWIFT: &[&str] = &[
    "as",
    "associatedtype",
    "break",
    "case",
    "catch",
    "class",
    "continue",
    "default",
    "defer",
    "deinit",
    "do",
    "else",
    "enum",
    "extension",
    "fallthrough",
    "false",
    "fileprivate",
    "for",
    "func",
    "guard",
    "if",
    "import",
    "in",
    "init",
    "inout",
    "internal",
    "is",
    "let",
    "nil",
    "open",
    "private",
    "public",
    "rethrows",
    "return",
    "self",
    "Self",
    "static",
    "struct",
    "subscript",
    "super",
    "switch",
    "throw",
    "throws",
    "true",
    "try",
    "typealias",
    "var",
    "where",
    "while",
    "protocol",
    "actor",
    "some",
    "any",
];
const BASH: &[&str] = &[
    "if", "then", "else", "elif", "fi", "for", "while", "until", "do", "done", "case", "esac",
    "in", "function", "return", "local", "export", "readonly", "declare", "shift", "echo",
    "printf", "source", "eval", "exec", "set", "unset", "trap", "exit", "break", "continue", "cd",
    "test", "true", "false",
];
const SQL: &[&str] = &[
    "select",
    "from",
    "where",
    "insert",
    "into",
    "values",
    "update",
    "set",
    "delete",
    "create",
    "table",
    "drop",
    "alter",
    "add",
    "join",
    "inner",
    "left",
    "right",
    "outer",
    "on",
    "group",
    "by",
    "order",
    "having",
    "limit",
    "offset",
    "as",
    "and",
    "or",
    "not",
    "null",
    "in",
    "is",
    "like",
    "between",
    "union",
    "all",
    "distinct",
    "count",
    "sum",
    "avg",
    "min",
    "max",
    "primary",
    "key",
    "foreign",
    "references",
    "index",
    "view",
    "case",
    "when",
    "then",
    "else",
    "end",
    "exists",
    "default",
    "unique",
    "constraint",
    "begin",
    "commit",
    "rollback",
    "transaction",
];

#[derive(Clone, Copy)]
struct Lang {
    keywords: &'static [&'static str],
    line_comment: &'static [&'static str],
    block_comment: bool,
    /// `'` opens a string (Rust: only `'x'` char literals).
    single_quote: bool,
    /// `` ` `` template strings.
    template: bool,
    /// `"""…"""` strings.
    triple_quote: bool,
    /// Highlight `key` before `:`/`=` (json/yaml/toml).
    key_highlight: bool,
}

fn lang(name: &str) -> Lang {
    let slash = Lang {
        keywords: &[],
        line_comment: &["//"],
        block_comment: true,
        single_quote: true,
        template: false,
        triple_quote: false,
        key_highlight: false,
    };
    let hash = Lang {
        keywords: &[],
        line_comment: &["#"],
        block_comment: false,
        single_quote: true,
        template: false,
        triple_quote: false,
        key_highlight: false,
    };
    match name {
        "rust" | "rs" => Lang {
            keywords: RUST,
            single_quote: false, // lifetimes; `'x'` handled specially
            ..slash
        },
        "javascript" | "js" | "typescript" | "ts" | "jsx" | "tsx" | "mjs" | "cjs" => Lang {
            keywords: JS,
            template: true,
            ..slash
        },
        "python" | "py" => Lang {
            keywords: PY,
            triple_quote: true,
            ..hash
        },
        "go" | "golang" => Lang {
            keywords: GO,
            single_quote: false,
            ..slash
        },
        "c" | "h" | "cpp" | "c++" | "cc" | "cxx" | "hpp" => Lang {
            keywords: C,
            ..slash
        },
        "java" | "scala" => Lang {
            keywords: JAVA,
            ..slash
        },
        "kotlin" | "kt" | "kts" => Lang {
            keywords: KOTLIN,
            ..slash
        },
        "swift" => Lang {
            keywords: SWIFT,
            single_quote: false,
            ..slash
        },
        "sh" | "bash" | "zsh" | "shell" | "fish" => Lang {
            keywords: BASH,
            ..hash
        },
        "ruby" | "rb" => Lang {
            keywords: &[],
            ..hash
        },
        "sql" => Lang {
            keywords: SQL,
            line_comment: &["--"],
            ..slash
        },
        "json" | "yaml" | "yml" | "toml" => Lang {
            keywords: &[],
            line_comment: if matches!(name, "json") { &[] } else { &["#"] },
            key_highlight: true,
            ..slash
        },
        _ => Lang {
            keywords: &[],
            line_comment: &["//", "#"],
            ..slash
        },
    }
}

/// Ranges in `src` tagged KEYWORD / STRING / COMMENT / NUMBER. Strings and
/// comments win over keywords; keywords require word boundaries. Byte-boundary
/// safe on UTF-8 (scans `char_indices`).
pub(crate) fn highlight(lang_name: &str, src: &str) -> Vec<(Range<usize>, u16)> {
    let l = lang(&lang_name.to_lowercase());
    let chars: Vec<(usize, char)> = src.char_indices().collect();
    let end = src.len();
    let byte = |i: usize| chars.get(i).map_or(end, |&(b, _)| b);
    let mut out: Vec<(Range<usize>, u16)> = Vec::new();
    let mut spans: Vec<Range<usize>> = Vec::new(); // string/comment — never re-scan
    let mut i = 0;
    while i < chars.len() {
        let (b, c) = chars[i];
        // Line comments.
        if l.line_comment.iter().any(|p| src[b..].starts_with(p)) {
            let mut j = i;
            while j < chars.len() && chars[j].1 != '\n' {
                j += 1;
            }
            let r = b..byte(j);
            out.push((r.clone(), COMMENT));
            spans.push(r);
            i = j;
            continue;
        }
        // Block comment /* */.
        if l.block_comment && src[b..].starts_with("/*") {
            let close = src[b + 2..]
                .find("*/")
                .map(|k| b + 2 + k + 2)
                .unwrap_or(end);
            let r = b..close;
            out.push((r.clone(), COMMENT));
            spans.push(r);
            i = chars.partition_point(|&(bi, _)| bi < close);
            continue;
        }
        // Python """…""" string.
        if l.triple_quote && src[b..].starts_with("\"\"\"") {
            let close = src[b + 3..]
                .find("\"\"\"")
                .map(|k| b + 3 + k + 3)
                .unwrap_or(end);
            let r = b..close;
            out.push((r.clone(), STRING));
            spans.push(r);
            i = chars.partition_point(|&(bi, _)| bi < close);
            continue;
        }
        // Strings.
        if c == '"'
            || (c == '\'' && (l.single_quote || is_rust_char(src, b)))
            || (c == '`' && l.template)
        {
            let q = c;
            let mut j = i + 1;
            while j < chars.len() {
                let (_, d) = chars[j];
                if d == '\\' {
                    j += 2;
                    continue;
                }
                if d == q || (q != '`' && d == '\n') {
                    j += (d == q) as usize;
                    break;
                }
                j += 1;
            }
            let r = b..byte(j);
            out.push((r.clone(), STRING));
            spans.push(r);
            i = j;
            continue;
        }
        i += 1;
    }
    // Second pass: numbers, keywords and json/yaml keys outside spans.
    let in_span = |b: usize| spans.iter().any(|s| s.contains(&b));
    let ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    i = 0;
    while i < chars.len() {
        let (b, c) = chars[i];
        if in_span(b) {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() {
            let mut j = i + 1;
            while j < chars.len() {
                let d = chars[j].1;
                if d.is_ascii_alphanumeric() || d == '_' || d == '.' {
                    j += 1;
                } else {
                    break;
                }
            }
            out.push((b..byte(j), NUMBER));
            i = j;
            continue;
        }
        if ident(c) && (i == 0 || !ident(chars[i - 1].1)) {
            let mut j = i + 1;
            while j < chars.len() && ident(chars[j].1) {
                j += 1;
            }
            let e = byte(j);
            let word = &src[b..e];
            let is_key = l.key_highlight
                && src[e..]
                    .trim_start_matches([' ', '\t'])
                    .starts_with([':', '=']);
            if l.keywords.contains(&word) || is_key {
                out.push((b..e, KEYWORD));
            }
            i = j;
            continue;
        }
        i += 1;
    }
    out.sort_by_key(|(r, _)| r.start);
    out
}

/// Rust: `'` starts a string only for `'x'` char literals (not lifetimes).
fn is_rust_char(src: &str, b: usize) -> bool {
    let rest = &src[b + 1..];
    let mut it = rest.char_indices();
    match it.next() {
        Some((_, '\\')) => rest.get(2..).is_some_and(|r| r.starts_with('\'')),
        Some(_) => match it.next() {
            Some((k, _)) => rest[k..].starts_with('\''),
            None => false,
        },
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(src: &str, hits: &[(Range<usize>, u16)], s: &str) -> u16 {
        let b = src.find(s).unwrap();
        hits.iter()
            .find(|(r, _)| r.contains(&b))
            .map(|(_, f)| *f)
            .unwrap_or(0)
    }

    #[test]
    fn rust_strings_and_comments() {
        let src = "let x = \"a // not comment\"; // real";
        let h = highlight("rust", src);
        assert_eq!(at(src, &h, "let"), KEYWORD);
        assert_eq!(at(src, &h, "x ="), 0);
        assert_eq!(at(src, &h, "a // not comment"), STRING);
        assert_eq!(at(src, &h, "real"), COMMENT);
        // Rust lifetime is not a string.
        let h = highlight("rust", "fn f<'a>(x: &'a str) {}");
        assert!(h.iter().all(|(_, f)| *f != STRING));
        let h = highlight("rust", "let c = 'x';");
        assert_eq!(at("let c = 'x';", &h, "'x'"), STRING);
    }

    #[test]
    fn python_comment_string_keyword() {
        let src = "# c\ns = 'x'\ndef f(): 42";
        let h = highlight("python", src);
        assert_eq!(at(src, &h, "# c"), COMMENT);
        assert_eq!(at(src, &h, "'x'"), STRING);
        assert_eq!(at(src, &h, "def"), KEYWORD);
        assert_eq!(at(src, &h, "42"), NUMBER);
        let h = highlight("python", "t = \"\"\"docstring\"\"\"");
        assert_eq!(at("t = \"\"\"docstring\"\"\"", &h, "docstring"), STRING);
    }

    #[test]
    fn json_keys_are_keywords() {
        let src = "{\"name\": 1, \"aéb\": \"v\"}";
        let h = highlight("json", src);
        assert_eq!(at(src, &h, "name"), STRING); // key is inside quotes
        assert_eq!(at(src, &h, "1"), NUMBER);
        let h = highlight("yaml", "key: value\nother: 2");
        assert_eq!(at("key: value\nother: 2", &h, "key"), KEYWORD);
        assert_eq!(at("key: value\nother: 2", &h, "other"), KEYWORD);
        assert_eq!(at("key: value\nother: 2", &h, "2"), NUMBER);
    }

    #[test]
    fn unknown_lang_keeps_generic() {
        let h = highlight("cobol", "x = \"s\" // c\n42");
        assert_eq!(at("x = \"s\" // c\n42", &h, "s\""), STRING);
        assert_eq!(at("x = \"s\" // c\n42", &h, "c"), COMMENT);
        assert_eq!(at("x = \"s\" // c\n42", &h, "42"), NUMBER);
    }

    #[test]
    fn utf8_boundaries() {
        let src = "s = \"é x\" # é\névar = 1";
        let h = highlight("python", src);
        for (r, _) in &h {
            let _ = &src[r.clone()]; // must not panic → boundaries valid
        }
        assert_eq!(at(src, &h, "é x"), STRING);
        assert_eq!(at(src, &h, "# é"), COMMENT);
        assert_eq!(at(src, &h, "1"), NUMBER);
    }
}
