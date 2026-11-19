use std::ops::Range;

use rune::{Color, Cx, El, TextHighlight, TextStyle, col, text};

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
            out.push(Decoration::Marker(marker, glyph));
        }
    }
    out
}

pub fn layers(s: &State, cx: &Cx<State>, p: Palette) -> (Vec<El>, Vec<El>, Vec<TextHighlight>) {
    let Some(doc) = s.document() else {
        return Default::default();
    };
    let Some(a) = doc.analysis.get(doc.revision) else {
        return Default::default();
    };
    let selected = doc.edit.anchor.min(doc.edit.focus)..doc.edit.anchor.max(doc.edit.focus);
    let plan = plan(&doc.edit.text, a, selected, s.settings.enabled("conceal"));
    let origin = cx.layout("editor");
    let mut back = vec![];
    let mut front = vec![];
    let mut highlights = vec![];
    for (index, decoration) in plan.into_iter().enumerate() {
        let key = format!("md-decoration-{index}");
        highlights.push(
            TextHighlight::new(decoration.range(), Color::TRANSPARENT)
                .pad(0.)
                .key(key.clone()),
        );
        let Some(([x, y, w, _], [dx, dy, _, dh])) = origin.zip(cx.layout(&key)) else {
            continue;
        };
        let layer = match decoration {
            Decoration::Code(_) => col()
                .absolute(-12., dy - y)
                .size(w + 24., dh)
                .clip_rounded(6.)
                .fill(rgb(p.code_bg)),
            Decoration::Quote(_) => col().absolute(-12., dy - y).size(2., dh).fill(rgb(p.line)),
            Decoration::Marker(_, glyph) => {
                front.push(
                    text(glyph)
                        .absolute(dx - x, dy - y)
                        .font(TextStyle::new(s.settings.number("editor_size"), 400.).lh(dh))
                        .color(rgb(p.dim))
                        .inert(),
                );
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
