use std::ops::Range;
use std::path::{Component, Path, PathBuf};

use crate::core::md::{Analysis, CODE};

#[derive(Debug, PartialEq)]
pub enum Target {
    Wiki { name: String, heading: String },
    Local { path: PathBuf, heading: String },
    External(String),
}

pub fn at(
    source: &str,
    caret: usize,
    analysis: &Analysis,
    root: &Path,
    current: &Path,
) -> Option<Target> {
    if let Some(link) = analysis
        .wiki_links
        .iter()
        .find(|link| link.range.contains(&caret))
    {
        let (name, heading) = split(&source[link.target.clone()]);
        return Some(Target::Wiki {
            name: name.into(),
            heading: heading.into(),
        });
    }
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_md::INLINE_LANGUAGE.into())
        .ok()?;
    let tree = parser.parse(source, None)?;
    fn destination<'a>(node: tree_sitter::Node<'a>, caret: usize) -> Option<Range<usize>> {
        if caret < node.start_byte() || caret >= node.end_byte() {
            return None;
        }
        if node.kind() == "inline_link" {
            return node
                .children(&mut node.walk())
                .find(|c| c.kind() == "link_destination")
                .map(|n| n.byte_range());
        }
        node.children(&mut node.walk())
            .find_map(|child| destination(child, caret))
    }
    let range = destination(tree.root_node(), caret)?;
    if analysis
        .flags
        .get(range.start)
        .is_some_and(|f| f & CODE != 0)
        || matches!(
            analysis.lines[analysis.line_of(range.start)].1,
            crate::core::md::Kind::Code
        )
    {
        return None;
    }
    destination_target(&source[range], root, current)
}

fn split(target: &str) -> (&str, &str) {
    target.split_once('#').unwrap_or((target, ""))
}

pub fn destination_target(target: &str, root: &Path, current: &Path) -> Option<Target> {
    let mut unescaped = String::with_capacity(target.len());
    let mut chars = target.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek().is_some_and(char::is_ascii_punctuation) {
            unescaped.push(chars.next()?);
        } else {
            unescaped.push(c);
        }
    }
    let target = unescaped.as_str();
    let target = target.trim().trim_start_matches('<').trim_end_matches('>');
    if target.starts_with("https://") || target.starts_with("http://") {
        return Some(Target::External(target.into()));
    }
    let (path, heading) = split(target);
    let decode = |text: &str| {
        let mut bytes = Vec::new();
        let mut input = text.bytes();
        while let Some(b) = input.next() {
            if b == b'%' {
                let hex = |b: u8| (b as char).to_digit(16).map(|n| n as u8);
                bytes.push(hex(input.next()?)? * 16 + hex(input.next()?)?);
            } else {
                bytes.push(b);
            }
        }
        String::from_utf8(bytes).ok()
    };
    let heading = decode(heading)?;
    if path.is_empty() {
        return Some(Target::Local {
            path: current.into(),
            heading,
        });
    }
    let mut resolved = PathBuf::new();
    for c in current.parent()?.join(decode(path)?).components() {
        match c {
            Component::ParentDir => {
                resolved.pop();
            }
            Component::CurDir => {}
            c => resolved.push(c.as_os_str()),
        }
    }
    if !resolved.starts_with(root)
        || !resolved
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("md"))
    {
        return None;
    }
    Some(Target::Local {
        path: resolved,
        heading,
    })
}

pub fn heading_range(source: &str, heading: &str) -> Range<usize> {
    if heading.is_empty() {
        return 0..0;
    }
    let normalize = |s: &str| {
        s.trim()
            .trim_end_matches('#')
            .trim()
            .to_lowercase()
            .replace(' ', "-")
    };
    let target = normalize(heading);
    let analysis = crate::core::md::Analyzer::new().analyze(source);
    for (range, kind) in analysis.lines {
        if !matches!(kind, crate::core::md::Kind::Heading(_)) {
            continue;
        }
        let trimmed = source[range.clone()].trim_start();
        if normalize(trimmed.trim_start_matches('#')) == target {
            return range.start..range.start;
        }
    }
    0..0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn heading_lookup_ignores_code_and_uses_real_markdown_headings() {
        let source = "intro\n```md\n# Part\n```\n\n    # Part\n\n#Part\n\n## Part\n";
        let offset = source.rfind("## Part").unwrap();
        assert_eq!(heading_range(source, "Part"), offset..offset);
        let setext = "intro\n\nPart\n----\n";
        assert_eq!(heading_range(setext, "Part"), 7..7);
        assert_eq!(heading_range("```\n# Part\n```\n", "Part"), 0..0);
        assert_eq!(heading_range("", "Part"), 0..0);
        assert_eq!(heading_range(source, "missing"), 0..0);
    }
    #[test]
    fn indented_code_links_are_not_followable() {
        for source in [
            "    [link](Note.md)",
            "paragraph\n\n    [link](https://example.com)",
            "    [[Note]]",
        ] {
            let analysis = crate::core::md::Analyzer::new().analyze(source);
            assert!(
                at(
                    source,
                    source.find('[').unwrap() + 2,
                    &analysis,
                    Path::new("/notes"),
                    Path::new("/notes/current.md")
                )
                .is_none(),
                "{source}"
            );
        }
    }
    #[test]
    fn markdown_destinations_unescape_punctuation_before_percent_decoding() {
        let source = r"[note](Note\(1\)%20draft.md#My\!%20heading)";
        let analysis = crate::core::md::Analyzer::new().analyze(source);
        assert_eq!(
            at(
                source,
                2,
                &analysis,
                Path::new("/notes"),
                Path::new("/notes/current.md")
            ),
            Some(Target::Local {
                path: "/notes/Note(1) draft.md".into(),
                heading: "My! heading".into(),
            })
        );
        assert_eq!(
            destination_target(
                r"https://example.com/a\(b\)",
                Path::new("/notes"),
                Path::new("/notes/current.md")
            ),
            Some(Target::External("https://example.com/a(b)".into()))
        );
        assert_eq!(
            destination_target(
                r"Note%5C(1%5C).md",
                Path::new("/notes"),
                Path::new("/notes/current.md")
            ),
            Some(Target::Local {
                path: r"/notes/Note\(1\).md".into(),
                heading: "".into()
            })
        );
    }
    #[test]
    fn follows_wiki_alias_headings_markdown_and_urls_but_not_code() {
        for (source, caret, expected) in [
            (
                "[[Café|alias]]",
                3,
                Some(Target::Wiki {
                    name: "Café".into(),
                    heading: "".into(),
                }),
            ),
            (
                "[[Note#My heading]]",
                4,
                Some(Target::Wiki {
                    name: "Note".into(),
                    heading: "My heading".into(),
                }),
            ),
            (
                "[note](Other%20Note.md#Part)",
                2,
                Some(Target::Local {
                    path: "/notes/Other Note.md".into(),
                    heading: "Part".into(),
                }),
            ),
            (
                "[site](https://example.com)",
                2,
                Some(Target::External("https://example.com".into())),
            ),
            ("`[[Code]]`", 4, None),
            ("```\n[a](b.md)\n```", 7, None),
            ("plain", 0, None),
            ("[escape](../../outside.md)", 2, None),
        ] {
            let analysis = crate::core::md::Analyzer::new().analyze(source);
            assert_eq!(
                at(
                    source,
                    caret,
                    &analysis,
                    Path::new("/notes"),
                    Path::new("/notes/current.md")
                ),
                expected,
                "{source}"
            );
        }
        assert_eq!(
            heading_range("# Start\r\n## My heading\r\n", "my-heading"),
            9..9
        );
        assert_eq!(heading_range("", "absent"), 0..0);
        assert_eq!(
            destination_target("bad%XX.md", Path::new("/notes"), Path::new("/notes/a.md")),
            None
        );
    }
}
