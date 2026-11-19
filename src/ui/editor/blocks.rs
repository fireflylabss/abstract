use std::ops::Range;

use rune::{Color, Cx, El, Icon, Span, TextHighlight, col, icon, row};

use crate::core::md::{Analysis, Bullet, Kind};
use crate::ui::{
    app::State,
    theme::{Palette, rgb},
};

#[derive(Clone, Debug, PartialEq)]
pub enum Decoration {
    Code(Range<usize>),
    Quote(Range<usize>),
    Marker(Range<usize>, &'static str),
}
impl Decoration {
    fn range(&self) -> Range<usize> {
        match self {
            Self::Code(r) | Self::Quote(r) | Self::Marker(r, _) => r.clone(),
        }
    }
}

pub fn plan(
    source: &str,
    analysis: &Analysis,
    selected: Range<usize>,
    conceal: bool,
) -> Vec<Decoration> {
    if source.len() != analysis.flags.len() {
        return vec![];
    }
    let mut out = vec![];
    for (line, kind) in &analysis.lines {
        if *kind == Kind::Code {
            if let Some(Decoration::Code(previous)) = out.last_mut()
                && previous.end + 1 == line.start
            {
                previous.end = line.end;
            } else {
                out.push(Decoration::Code(line.clone()));
            }
        } else if *kind == Kind::Quote && !line.is_empty() {
            out.push(Decoration::Quote(line.clone()));
        }
    }
    if conceal {
        for item in &analysis.items {
            let end = source[item.marker.start..]
                .find('\n')
                .map_or(source.len(), |n| item.marker.start + n);
            let start = source[..item.marker.start].rfind('\n').map_or(0, |n| n + 1);
            if start <= selected.end && selected.start <= end {
                continue;
            }
            let glyph = match item.bullet {
                Bullet::Dot if item.depth > 0 => "◦",
                Bullet::Dot => "•",
                Bullet::Task { checked: false } => "☐",
                Bullet::Task { checked: true } => "☑",
                Bullet::Ordered => continue,
            };
            let mut marker = item.marker.clone();
            if matches!(item.bullet, Bullet::Task { .. }) {
                let index = analysis
                    .tasks
                    .partition_point(|t| t.marker.start < marker.end);
                if let Some(task) = analysis.tasks.get(index).filter(|t| t.marker.start <= end) {
                    marker.end = task.marker.end;
                }
            }
            while matches!(source.as_bytes().get(marker.end), Some(b' ' | b'\t')) {
                marker.end += 1;
            }
            marker.start = start;
            out.push(Decoration::Marker(marker, glyph));
        }
    }
    out
}

pub fn marker_span(source: &str, range: Range<usize>, size: f32) -> Span {
    let prefix = &source[range.clone()];
    let columns = |c| if c == '\t' { 4 } else { 1 };
    let indent: usize = prefix
        .chars()
        .take_while(|c| c.is_whitespace())
        .map(columns)
        .sum();
    let width = (20. + (indent / 2).min(6) as f32 * 28.) * size / 17.;
    let count: usize = prefix.chars().map(columns).sum();
    Span::new(range.start, range.end)
        .mono()
        .size(width / (count.max(1) as f32 * 0.6))
        .color(Color::TRANSPARENT)
}

pub fn task_text(source: &str, marker: &Range<usize>) -> Range<usize> {
    let mut end = source[marker.end..]
        .find('\n')
        .map_or(source.len(), |n| marker.end + n);
    let mut start = marker.end;
    while start < end && matches!(source.as_bytes()[start], b' ' | b'\t') {
        start += 1;
    }
    while end > start && matches!(source.as_bytes()[end - 1], b' ' | b'\t' | b'\r') {
        end -= 1;
    }
    start..end
}

static CHECK: Icon = Icon {
    name: "task-check",
    view: 24.,
    body: include_str!("../../../assets/icons/tick-02.svg"),
};

fn code_content(
    source: &str,
    range: Range<usize>,
    selected: &Range<usize>,
    conceal: bool,
) -> (Range<usize>, f32) {
    let raw = &source[range.clone()];
    let fence =
        |line: &str| line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~");
    let first_end = raw.find('\n').map_or(range.end, |n| range.start + n);
    let last_start = raw.rfind('\n').map_or(range.start, |n| range.start + n + 1);
    let touches = |start, end| start <= selected.end && selected.start <= end;
    if !conceal || !fence(&source[range.start..first_end]) || touches(range.start, first_end) {
        return (range, 0.);
    }
    let start = (first_end + 1).min(range.end);
    let end = if last_start > range.start && fence(&source[last_start..range.end]) {
        if touches(last_start, range.end) {
            return (range, 0.);
        }
        last_start.saturating_sub(1).max(start)
    } else {
        range.end
    };
    (start..end, 12.)
}

pub fn layers(s: &State, cx: &Cx<State>, p: Palette) -> (Vec<El>, Vec<El>, Vec<TextHighlight>) {
    let Some(doc) = s.document() else {
        return Default::default();
    };
    let Some(a) = doc.analysis.get(doc.revision) else {
        return Default::default();
    };
    let selected = doc.edit.anchor.min(doc.edit.focus)..doc.edit.anchor.max(doc.edit.focus);
    let plan = plan(
        &doc.edit.text,
        a,
        selected.clone(),
        s.settings.enabled("conceal"),
    );
    let origin = cx.layout("editor");
    let mut back = vec![];
    let mut front = vec![];
    let mut highlights = vec![];
    for (index, decoration) in plan.into_iter().enumerate() {
        let key = format!("md-decoration-{index}");
        let (range, padding) = if let Decoration::Code(range) = &decoration {
            code_content(
                &doc.edit.text,
                range.clone(),
                &selected,
                s.settings.enabled("conceal"),
            )
        } else {
            (decoration.range(), 0.)
        };
        highlights.push(
            TextHighlight::new(range, Color::TRANSPARENT)
                .pad(0.)
                .key(key.clone()),
        );
        let Some(([x, y, w, _], [dx, dy, dw, dh])) = origin.zip(cx.layout(&key)) else {
            continue;
        };
        let layer = match decoration {
            Decoration::Code(_) => col()
                .absolute(-12., dy - y - padding)
                .size(w + 24., dh + padding * 2.)
                .clip_rounded(6.)
                .fill(rgb(p.code_bg)),
            Decoration::Quote(_) => col().absolute(-12., dy - y).size(2., dh).fill(rgb(p.line)),
            Decoration::Marker(_, glyph) => {
                let scale = s.settings.number("editor_size") / 17.;
                let mut marker = row()
                    .key(format!("md-marker-{index}"))
                    .absolute(dx - x + dw - 20. * scale, dy - y + (dh - 14. * scale) / 2.)
                    .size(14. * scale, 14. * scale)
                    .items_center()
                    .justify_center();
                if matches!(glyph, "☐" | "☑") {
                    marker = marker.radius(3. * scale);
                    if glyph == "☑" {
                        marker = marker
                            .fill(rgb(p.fg))
                            .child(icon(&CHECK, 12. * scale).color(rgb(p.bg)));
                    } else {
                        marker = marker.border(1., rgb(p.dim));
                    }
                } else {
                    let dot = col().size(4. * scale, 4. * scale).radius(2. * scale);
                    marker = marker.child(if glyph == "◦" {
                        dot.border(1., rgb(p.dim))
                    } else {
                        dot.fill(rgb(p.dim))
                    });
                }
                front.push(marker.inert());
                continue;
            }
        };
        back.push(layer.inert());
    }
    (back, front, highlights)
}

#[cfg(test)]
#[path = "blocks_tests.rs"]
mod tests;
