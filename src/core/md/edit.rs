//! Markdown commands expressed as one replacement and its resulting selection.

use std::ops::Range;

use crate::core::lines;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub focus: usize,
}

impl Selection {
    fn range(self) -> Range<usize> {
        self.anchor.min(self.focus)..self.anchor.max(self.focus)
    }

    fn from_range(range: Range<usize>, reversed: bool) -> Self {
        if reversed {
            Self {
                anchor: range.end,
                focus: range.start,
            }
        } else {
            Self {
                anchor: range.start,
                focus: range.end,
            }
        }
    }
}

pub enum Command {
    Enter,
    Indent { outdent: bool },
    ToggleTask,
    Wrap(&'static str),
    Link,
    MoveLines { down: bool },
    DuplicateLines,
    SelectNext,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Edit {
    pub range: Range<usize>,
    pub text: String,
    pub selection: Selection,
}

impl Edit {
    fn insert(range: Range<usize>, text: String) -> Self {
        let caret = range.start + text.len();
        Self {
            range,
            text,
            selection: Selection {
                anchor: caret,
                focus: caret,
            },
        }
    }
}

pub fn command(source: &str, selection: Selection, command: Command) -> Option<Edit> {
    if !source.is_char_boundary(selection.anchor) || !source.is_char_boundary(selection.focus) {
        return None;
    }
    let sel = selection.range();
    let reversed = selection.anchor > selection.focus;
    match command {
        Command::Enter => Some(enter(source, sel)),
        Command::Indent { outdent } => indent(source, selection, outdent),
        Command::ToggleTask => {
            let line = line(source, selection.focus);
            if fenced(source, line.start) {
                return None;
            }
            let task = marker(source[line.clone()].trim_end_matches('\r'))?.task? + line.start;
            Some(Edit {
                range: task..task + 1,
                text: if &source[task..task + 1] == " " {
                    "x"
                } else {
                    " "
                }
                .into(),
                selection,
            })
        }
        Command::Wrap(marker) if !marker.is_empty() => {
            Some(wrap(source, selection, marker, marker))
        }
        Command::Wrap(_) => None,
        Command::Link => Some(link(source, selection)),
        Command::SelectNext => Some(Edit {
            range: 0..0,
            text: String::new(),
            selection: Selection::from_range(lines::select_next(source, &sel)?, false),
        }),
        Command::MoveLines { down } => {
            let (range, text, selected) = lines::move_sel(source, &sel, down)?;
            Some(Edit {
                range,
                text,
                selection: Selection::from_range(selected, reversed),
            })
        }
        Command::DuplicateLines => {
            let (range, text, selected) = lines::duplicate_sel(source, &sel);
            Some(Edit {
                range,
                text,
                selection: Selection::from_range(selected, reversed),
            })
        }
    }
}

fn line(source: &str, at: usize) -> Range<usize> {
    let start = source.as_bytes()[..at]
        .iter()
        .rposition(|b| *b == b'\n')
        .map_or(0, |i| i + 1);
    let end = source.as_bytes()[at..]
        .iter()
        .position(|b| *b == b'\n')
        .map_or(source.len(), |i| at + i);
    start..end
}

fn padding(source: &str) -> usize {
    source
        .bytes()
        .take_while(|b| matches!(b, b' ' | b'\t'))
        .count()
}

struct Marker {
    len: usize,
    next: String,
    task: Option<usize>,
    list: bool,
}

fn marker(line: &str) -> Option<Marker> {
    let indent = padding(line);
    let pad = &line[..indent];
    let rest = &line[indent..];
    let first = *rest.as_bytes().first()?;
    if first == b'>' {
        let mut end = 0;
        while rest.as_bytes().get(end) == Some(&b'>') {
            end += 1;
            end += padding(&rest[end..]);
        }
        return Some(Marker {
            len: indent + end,
            next: format!("{pad}{}", &rest[..end]),
            task: None,
            list: false,
        });
    }
    let (end, next) = if matches!(first, b'-' | b'*' | b'+') {
        (1, format!("{} ", first as char))
    } else {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 || !matches!(rest.as_bytes().get(digits), Some(b'.' | b')')) {
            return None;
        }
        let n = rest[..digits].parse::<u64>().ok()?.checked_add(1)?;
        (
            digits + 1,
            format!("{n}{} ", rest.as_bytes()[digits] as char),
        )
    };
    if rest.len() > end && padding(&rest[end..]) == 0 {
        return None;
    }
    let body = end + padding(&rest[end..]);
    let task = rest
        .as_bytes()
        .get(body..body + 3)
        .filter(|s| s[0] == b'[' && matches!(s[1], b' ' | b'x' | b'X') && s[2] == b']')
        .filter(|_| rest.len() == body + 3 || padding(&rest[body + 3..]) > 0)
        .map(|_| body + 1);
    let len = if task.is_some() {
        body + 3 + padding(&rest[body + 3..])
    } else {
        body
    };
    Some(Marker {
        len: indent + len,
        next: format!("{pad}{next}{}", if task.is_some() { "[ ] " } else { "" }),
        task: task.map(|t| indent + t),
        list: true,
    })
}

fn fenced(source: &str, until: usize) -> bool {
    let mut open = None;
    for line in source[..until].lines() {
        let s = line.trim_start_matches([' ', '\t']);
        let Some(first @ (b'`' | b'~')) = s.as_bytes().first().copied() else {
            continue;
        };
        let count = s.bytes().take_while(|b| *b == first).count();
        if count < 3 {
            continue;
        }
        match open {
            None => open = Some((first, count)),
            Some((ch, n)) if ch == first && count >= n && s[count..].trim().is_empty() => {
                open = None
            }
            _ => {}
        }
    }
    open.is_some()
}

fn enter(source: &str, sel: Range<usize>) -> Edit {
    let line = line(source, sel.start);
    let current = source[line.clone()].trim_end_matches('\r');
    let pad = &current[..padding(current)];
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    if !fenced(source, line.start)
        && let Some(marker) = marker(current)
    {
        if sel.is_empty() && current[marker.len..].trim().is_empty() {
            return Edit::insert(line.start..line.start + current.len(), String::new());
        }
        if sel.start >= line.start + marker.len {
            return Edit::insert(sel, format!("{newline}{}", marker.next));
        }
    }
    Edit::insert(sel, format!("{newline}{pad}"))
}

fn indent(source: &str, selection: Selection, outdent: bool) -> Option<Edit> {
    let sel = selection.range();
    let first = line(source, sel.start);
    let last = line(source, if sel.is_empty() { sel.end } else { sel.end - 1 });
    let block = first.start..last.end;
    let list = !fenced(source, first.start)
        && source[block.clone()]
            .lines()
            .any(|l| marker(l).is_some_and(|m| m.list));
    if !list {
        return (!outdent).then(|| Edit::insert(sel, "  ".into()));
    }
    let mut text = String::new();
    let mut mapped = selection;
    let mut at = block.start;
    let mut delta = 0;
    for l in source[block.clone()].split('\n') {
        if at > block.start {
            text.push('\n');
        }
        let remove = if outdent {
            if l.starts_with('\t') {
                1
            } else {
                l.bytes().take_while(|b| *b == b' ').count().min(2)
            }
        } else {
            0
        };
        let add = if outdent { "" } else { "  " };
        text.push_str(add);
        text.push_str(&l[remove..]);
        let shift = |pos: usize| -> isize {
            if pos < at {
                0
            } else {
                add.len() as isize - remove.min(pos - at) as isize
            }
        };
        mapped.anchor = mapped.anchor.checked_add_signed(shift(selection.anchor))?;
        mapped.focus = mapped.focus.checked_add_signed(shift(selection.focus))?;
        delta += add.len() as isize - remove as isize;
        at += l.len() + 1;
    }
    (delta != 0).then_some(Edit {
        range: block,
        text,
        selection: mapped,
    })
}

fn wrap(source: &str, selection: Selection, left: &str, right: &str) -> Edit {
    let sel = selection.range();
    let inner = &source[sel.clone()];
    let pair = |start: usize, end: usize| {
        let run = |at: usize| {
            source[..at]
                .bytes()
                .rev()
                .take_while(|b| *b == b'*')
                .count()
                + source[at..].bytes().take_while(|b| *b == b'*').count()
        };
        left != "*"
            || (sel.is_empty() && run(start) == 2)
            || (run(start) % 2 == 1 && run(end) % 2 == 1)
    };
    let (range, text, start) = if inner.len() >= left.len() + right.len()
        && inner.starts_with(left)
        && inner.ends_with(right)
        && pair(sel.start, sel.end)
    {
        (
            sel.clone(),
            inner[left.len()..inner.len() - right.len()].to_owned(),
            sel.start,
        )
    } else if source[..sel.start].ends_with(left)
        && source[sel.end..].starts_with(right)
        && pair(sel.start - left.len(), sel.end + right.len())
    {
        (
            sel.start - left.len()..sel.end + right.len(),
            inner.to_owned(),
            sel.start - left.len(),
        )
    } else {
        (
            sel.clone(),
            format!("{left}{inner}{right}"),
            sel.start + left.len(),
        )
    };
    let len = if range == sel && text.len() > inner.len() {
        inner.len()
    } else {
        text.len()
    };
    Edit {
        range,
        text,
        selection: Selection::from_range(start..start + len, selection.anchor > selection.focus),
    }
}

fn link(source: &str, selection: Selection) -> Edit {
    let sel = selection.range();
    let inner = &source[sel.clone()];
    if let Some((label, end)) = link_parts(inner)
        && end == inner.len()
    {
        let label = &inner[label];
        return Edit {
            range: sel.clone(),
            text: label.into(),
            selection: Selection::from_range(
                sel.start..sel.start + label.len(),
                selection.anchor > selection.focus,
            ),
        };
    }
    if source[..sel.start].ends_with('[')
        && let Some((label, end)) = link_parts(&source[sel.start - 1..])
        && sel.start - 1 + label.end == sel.end
    {
        return Edit {
            range: sel.start - 1..sel.start - 1 + end,
            text: inner.into(),
            selection: Selection::from_range(
                sel.start - 1..sel.end - 1,
                selection.anchor > selection.focus,
            ),
        };
    }
    Edit {
        range: sel.clone(),
        text: format!("[{inner}]()"),
        selection: Selection::from_range(
            sel.start + 1..sel.end + 1,
            selection.anchor > selection.focus,
        ),
    }
}

fn link_parts(source: &str) -> Option<(Range<usize>, usize)> {
    fn close(source: &str, open: u8, close: u8) -> Option<usize> {
        let mut depth = 0usize;
        let mut escaped = false;
        for (i, byte) in source.bytes().enumerate() {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == open {
                depth += 1;
            } else if byte == close {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i);
                }
            }
        }
        None
    }
    if !source.starts_with('[') {
        return None;
    }
    let label_end = close(source, b'[', b']')?;
    let url = &source[label_end + 1..];
    if !url.starts_with('(') {
        return None;
    }
    Some((1..label_end, label_end + 2 + close(url, b'(', b')')?))
}

#[cfg(test)]
mod tests;
