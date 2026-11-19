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
    {
        return None;
    }
    destination_target(&source[range], root, current)
}

fn split(target: &str) -> (&str, &str) {
    target.split_once('#').unwrap_or((target, ""))
}

pub fn destination_target(target: &str, root: &Path, current: &Path) -> Option<Target> {
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
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') && normalize(trimmed.trim_start_matches('#')) == target {
            return offset..offset;
        }
        offset += line.len();
    }
    0..0
}

#[cfg(test)]
mod tests {
    use super::*;
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
