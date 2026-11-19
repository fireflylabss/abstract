use rune::{Color, Span};

use crate::core::md::{self, Analysis, Kind};
use crate::ui::theme::{Palette, rgb, rgba};

#[cfg(test)]
pub fn decorate(source: &str, anchor: usize, focus: usize, a: &Analysis, p: Palette) -> Vec<Span> {
    decorate_with(
        source,
        anchor,
        focus,
        a,
        p,
        &crate::core::store::Settings::default(),
    )
}

pub fn decorate_with(
    source: &str,
    anchor: usize,
    focus: usize,
    a: &Analysis,
    p: Palette,
    settings: &crate::core::store::Settings,
) -> Vec<Span> {
    if a.flags.len() != source.len() {
        return vec![];
    }
    let selected = anchor.min(focus)..anchor.max(focus);
    let conceal = settings.enabled("conceal");
    let base = settings.number("editor_size") / 17.;
    let headings = settings.number("heading_scale");
    let spacing = settings.number("paragraph_spacing");
    let mut concealed = vec![false; source.len()];
    let mut stable = vec![false; source.len()];
    for item in &a.items {
        stable[item.marker.clone()].fill(true);
    }
    for task in &a.tasks {
        let mut end = task.marker.end;
        while matches!(source.as_bytes().get(end), Some(b' ' | b'\t')) {
            end += 1;
        }
        stable[task.marker.start..end].fill(true);
    }
    for (line, kind) in &a.lines {
        if *kind == Kind::Quote {
            for (offset, c) in source[line.clone()].char_indices() {
                if c == '>' || c.is_whitespace() {
                    stable[line.start + offset..line.start + offset + c.len_utf8()].fill(true);
                } else {
                    break;
                }
            }
        }
    }
    for c in &a.conceals {
        concealed[c.hidden.clone()].fill(true);
    }
    let mut out = Vec::new();
    for (line, kind) in &a.lines {
        let size = match kind {
            Kind::Heading(1) => 32.,
            Kind::Heading(2) => 25.,
            Kind::Heading(_) => 21.,
            _ => 17.,
        };
        let size = size
            * base
            * if matches!(kind, Kind::Heading(_)) {
                headings
            } else {
                1.
            };
        let size = if source[line.clone()].trim().is_empty() {
            size + spacing
        } else {
            size
        };
        let reveal = line.start <= selected.end && selected.start <= line.end;
        if spacing > 0. && line.is_empty() && source.as_bytes().get(line.end) == Some(&b'\n') {
            out.push(Span::new(line.end, line.end + 1).size(size));
        }
        let hide = |i: usize| {
            conceal
                && !reveal
                && !matches!(kind, Kind::Code | Kind::Table)
                && concealed[i]
                && !stable[i]
        };
        let mut pos = line.start;
        while pos < line.end {
            let flag = a.flags[pos];
            let hidden = hide(pos);
            let mut end = pos + source[pos..].chars().next().expect("character").len_utf8();
            while end < line.end
                && a.flags[end] == flag
                && hide(end) == hidden
                && stable[end] == stable[pos]
            {
                end += source[end..].chars().next().expect("character").len_utf8();
            }
            let mut span = Span::new(pos, end).size(if hidden { 0.1 } else { size });
            if matches!(kind, Kind::Heading(_)) || flag & md::BOLD != 0 {
                span = span.weight(700.).color(rgb(p.head));
            }
            if flag & md::ITALIC != 0 {
                span = span.italic();
            }
            if matches!(kind, Kind::Code | Kind::Table) || flag & md::CODE != 0 {
                span = span.mono().bg(rgb(p.inline_code_bg), 2.);
            }
            if *kind == Kind::Quote {
                span = span.color(rgb(p.quote));
            }
            if flag & md::LINK != 0 {
                span = span.color(rgb(p.callout[0])).underline(rgb(p.callout[0]));
            }
            if flag & (md::MARK | md::MUTED) != 0 || stable[pos] {
                span = span.color(rgb(p.muted));
            }
            if flag & md::KEYWORD != 0 {
                span = span.color(rgb(p.code_kw));
            }
            if flag & md::STRING != 0 {
                span = span.color(rgb(p.code_str));
            }
            if flag & md::COMMENT != 0 {
                span = span.color(rgb(p.code_comment));
            }
            if flag & md::NUMBER != 0 {
                span = span.color(rgb(p.code_num));
            }
            if flag & md::HIGHLIGHT != 0 {
                span = span.bg(rgba(p.highlight), 2.);
            }
            if hidden {
                span = span.color(Color::TRANSPARENT);
            }
            out.push(span);
            pos = end;
        }
    }
    out
}

#[cfg(test)]
#[path = "spans_tests.rs"]
mod tests;
