use std::io;
use std::sync::Arc;

use krilla::Document;
use krilla::color::rgb;
use krilla::geom::{PathBuilder, Point, Size, Transform};
use krilla::metadata::Metadata;
use krilla::page::PageSettings;
use krilla::paint::{Fill, Stroke};
use krilla::text::{Font, GlyphId, KrillaGlyph};

use super::model::*;
use super::{BODY_PT, BULLET_SPACE, FAINT, FOOTER_Y, INK, TEXT_W};

#[cfg(test)]
#[path = "writer_tests.rs"]
mod tests;

const PT_MM: f32 = 72. / 25.4;
const PAGE_W: f32 = 210. * PT_MM;
const PAGE_H: f32 = 297. * PT_MM;
const TOP: f32 = 18. * PT_MM;
const BOTTOM: f32 = PAGE_H - 20. * PT_MM;
const LEFT: f32 = 22. * PT_MM;

struct Face {
    font: Font,
    source: harfrust::FontRef<'static>,
    shaper: harfrust::ShaperData,
}
struct Fonts(Vec<Face>);

impl Fonts {
    fn load() -> io::Result<Self> {
        use super::*;
        let faces = [
            FONT_SANS,
            FONT_SANS_BOLD,
            FONT_SANS_ITALIC,
            FONT_SANS_BOLD_ITALIC,
            FONT_MONO,
            FONT_MONO_BOLD,
            FONT_MATH,
            FONT_MATH_ITALIC,
        ];
        faces
            .into_iter()
            .map(|data| {
                let font = Font::new(Arc::new(data.to_vec()).into(), 0)
                    .ok_or_else(|| io::Error::other("Invalid bundled PDF font"))?;
                let source = harfrust::FontRef::new(data).map_err(io::Error::other)?;
                let shaper = harfrust::ShaperData::new(&source);
                Ok(Face {
                    font,
                    source,
                    shaper,
                })
            })
            .collect::<io::Result<Vec<_>>>()
            .map(Self)
    }

    fn face(&self, style: Style) -> &Face {
        let index = match style.family.unwrap_or_default() {
            Family::Sans => usize::from(style.bold) + 2 * usize::from(style.italic),
            Family::Mono => 4 + usize::from(style.bold),
            Family::Math => 6 + usize::from(style.italic || style.bold),
        };
        &self.0[index]
    }

    fn shape(&self, text: &str, style: Style) -> Vec<KrillaGlyph> {
        let face = self.face(style);
        let font = face.shaper.shaper(&face.source).build();
        let mut buffer = harfrust::UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        let shaped = font.shape(buffer, harfrust::ShapeOptions::default());
        let infos = shaped.glyph_infos();
        let mut boundaries: Vec<_> = infos.iter().map(|i| i.cluster as usize).collect();
        boundaries.push(text.len());
        boundaries.sort_unstable();
        boundaries.dedup();
        let units = face.font.units_per_em();
        infos
            .iter()
            .zip(shaped.glyph_positions())
            .map(|(info, p)| {
                let start = info.cluster as usize;
                let end = boundaries[boundaries.partition_point(|b| *b <= start)];
                KrillaGlyph::new(
                    GlyphId::new(info.glyph_id),
                    p.x_advance as f32 / units,
                    p.x_offset as f32 / units,
                    p.y_offset as f32 / units,
                    p.y_advance as f32 / units,
                    start..end,
                    None,
                )
            })
            .collect()
    }

    fn width(&self, text: &str, style: Style) -> f32 {
        self.shape(text, style)
            .iter()
            .map(|g| g.x_advance)
            .sum::<f32>()
            * style.size.unwrap_or(BODY_PT) as f32
    }
}

enum Draw {
    Text {
        x: f32,
        y: f32,
        run: StyledString,
    },
    Line {
        from: (f32, f32),
        to: (f32, f32),
        color: Color,
    },
    Image {
        x: f32,
        y: f32,
        image: Image,
    },
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
struct Position {
    page: usize,
    y: f32,
}

struct Layout<'a> {
    fonts: &'a Fonts,
    pages: Vec<Vec<Draw>>,
    pos: Position,
}

impl Layout<'_> {
    fn draw(&mut self, draw: Draw) {
        while self.pages.len() <= self.pos.page {
            self.pages.push(vec![]);
        }
        self.pages[self.pos.page].push(draw);
    }

    fn reserve(&mut self, height: f32) {
        if self.pos.y + height > BOTTOM && self.pos.y > TOP {
            self.pos = Position {
                page: self.pos.page + 1,
                y: TOP,
            };
        }
    }

    fn gap(&mut self, height: f32) {
        self.reserve(height);
        self.pos.y += height;
    }

    fn line(&mut self, from: (f32, f32), to: (f32, f32), color: Color) {
        self.draw(Draw::Line { from, to, color });
    }

    fn paragraph(&mut self, p: &Paragraph, x: f32, width: f32) {
        if p.preformatted {
            self.preformatted(p, x, width);
            return;
        }
        let mut runs: Vec<StyledString> = vec![];
        let mut used = 0.;
        for run in &p.runs {
            for word in run.s.split_inclusive(char::is_whitespace) {
                let mut word = word;
                let mut size = self.fonts.width(word, run.style);
                if used + size > width && !runs.is_empty() {
                    self.text_line(std::mem::take(&mut runs), p.alignment, x, width);
                    used = 0.;
                }
                if runs.is_empty() {
                    word = word.trim_start();
                    size = self.fonts.width(word, run.style);
                }
                if size <= width {
                    if !word.is_empty() {
                        runs.push(StyledString::new(word, run.style));
                    }
                    used += size;
                    continue;
                }
                // Break long URLs/identifiers without slicing a UTF-8 character.
                for c in word.chars() {
                    let s = c.to_string();
                    let size = self.fonts.width(&s, run.style);
                    if used + size > width && !runs.is_empty() {
                        self.text_line(std::mem::take(&mut runs), p.alignment, x, width);
                        used = 0.;
                    }
                    runs.push(StyledString::new(s, run.style));
                    used += size;
                }
            }
        }
        if !runs.is_empty() {
            self.text_line(runs, p.alignment, x, width);
        }
    }

    fn preformatted(&mut self, p: &Paragraph, x: f32, width: f32) {
        let mut runs = vec![];
        let mut used = 0.;
        for run in &p.runs {
            let mut text = String::new();
            for c in run.s.chars() {
                let size = self.fonts.width(&c.to_string(), run.style);
                if used + size > width && used > 0. {
                    if !text.is_empty() {
                        runs.push(StyledString::new(std::mem::take(&mut text), run.style));
                    }
                    self.text_line(std::mem::take(&mut runs), p.alignment, x, width);
                    used = 0.;
                }
                text.push(c);
                used += size;
            }
            runs.push(StyledString::new(text, run.style));
        }
        self.text_line(runs, p.alignment, x, width);
    }

    fn text_line(&mut self, runs: Vec<StyledString>, align: Alignment, x: f32, width: f32) {
        let size = runs
            .iter()
            .map(|r| r.style.size.unwrap_or(BODY_PT))
            .max()
            .unwrap_or(BODY_PT) as f32;
        let line_height = size * 1.35;
        self.reserve(line_height);
        let used = runs
            .iter()
            .map(|r| self.fonts.width(&r.s, r.style))
            .sum::<f32>();
        let mut x = x + match align {
            Alignment::Left => 0.,
            Alignment::Center => (width - used).max(0.) / 2.,
            Alignment::Right => (width - used).max(0.),
        };
        for run in runs {
            let advance = self.fonts.width(&run.s, run.style);
            self.draw(Draw::Text {
                x,
                y: self.pos.y + size,
                run,
            });
            x += advance;
        }
        self.pos.y += line_height;
    }

    fn block(&mut self, block: &Block, x: f32, width: f32) {
        match block {
            Block::Group(blocks) => {
                for block in blocks {
                    self.block(block, x, width);
                }
            }
            Block::Paragraph(p) => self.paragraph(p, x, width),
            Block::Gap(lines) => self.gap(*lines as f32 * BODY_PT as f32 * 1.35),
            Block::Rule(color) => {
                self.reserve(PT_MM);
                self.line(
                    (x, self.pos.y + PT_MM / 2.),
                    (x + width, self.pos.y + PT_MM / 2.),
                    *color,
                );
                self.pos.y += PT_MM;
            }
            Block::Inset {
                inner,
                margins,
                frame,
                bar,
                color,
            } => {
                self.reserve(margins.top as f32 * PT_MM + BODY_PT as f32 * 1.35);
                let start = self.pos;
                self.gap(margins.top as f32 * PT_MM);
                self.block(
                    inner,
                    x + margins.left as f32 * PT_MM,
                    (width - (margins.left + margins.right) as f32 * PT_MM).max(1.),
                );
                self.gap(margins.bottom as f32 * PT_MM);
                self.border(start, x, width, *color, *frame, *bar);
            }
            Block::Bullet {
                inner,
                label,
                style,
            } => {
                self.reserve(BODY_PT as f32 * 1.35);
                let marker_w = self.fonts.width(label, *style);
                let indent =
                    (super::INDENT as f32 * PT_MM).max(marker_w + BULLET_SPACE as f32 * PT_MM);
                self.draw(Draw::Text {
                    x: x + indent - BULLET_SPACE as f32 * PT_MM - marker_w,
                    y: self.pos.y + BODY_PT as f32,
                    run: StyledString::new(label, *style),
                });
                self.block(inner, x + indent, (width - indent).max(1.));
            }
            Block::Table(table) => {
                let total = table.weights.iter().sum::<usize>().max(1) as f32;
                for row in &table.rows {
                    self.reserve(BODY_PT as f32 * 1.35 + 4. * PT_MM);
                    let start = self.pos;
                    let mut end = start;
                    let mut cell_x = x;
                    let mut spans = vec![];
                    for (cell, weight) in row.iter().zip(&table.weights) {
                        let cell_w = width * *weight as f32 / total;
                        self.pos = start;
                        self.block(cell, cell_x, cell_w);
                        if self.pos > end {
                            end = self.pos;
                        }
                        spans.push((cell_x, cell_w));
                        cell_x += cell_w;
                    }
                    self.pos = end;
                    for (x, w) in spans {
                        self.border(start, x, w, table.color, true, false);
                    }
                }
            }
            Block::Image(image) => {
                let scale = (width / (image.width as f32 * PT_MM)).min(1.);
                let mut image = image.clone();
                image.width *= scale as f64;
                image.height *= scale as f64;
                let w = image.width as f32 * PT_MM;
                let h = image.height as f32 * PT_MM;
                self.reserve(h);
                let x = x + match image.alignment {
                    Alignment::Center => (width - w) / 2.,
                    Alignment::Right => width - w,
                    Alignment::Left => 0.,
                };
                self.draw(Draw::Image {
                    x,
                    y: self.pos.y,
                    image,
                });
                self.pos.y += h;
            }
        }
    }

    fn border(
        &mut self,
        start: Position,
        x: f32,
        width: f32,
        color: Color,
        frame: bool,
        bar: bool,
    ) {
        let end = self.pos;
        for page in start.page..=end.page {
            self.pos = Position { page, y: TOP };
            let top = if page == start.page { start.y } else { TOP };
            let bottom = if page == end.page { end.y } else { BOTTOM };
            if frame {
                self.line((x, top), (x + width, top), color);
                self.line((x, bottom), (x + width, bottom), color);
                self.line((x + width, top), (x + width, bottom), color);
                self.line((x, top), (x, bottom), color);
            } else if bar {
                self.line((x + 1.2 * PT_MM, top), (x + 1.2 * PT_MM, bottom), color);
            }
        }
        self.pos = end;
    }
}

fn color(color: Color) -> rgb::Color {
    let Color::Rgb(r, g, b) = color;
    rgb::Color::new(r, g, b)
}

pub(super) fn export(body: LinearLayout, title: &str) -> io::Result<Vec<u8>> {
    let fonts = Fonts::load()?;
    let mut layout = Layout {
        fonts: &fonts,
        pages: vec![vec![]],
        pos: Position { page: 0, y: TOP },
    };
    layout.block(&body.into(), LEFT, TEXT_W as f32 * PT_MM);
    let mut document = Document::new();
    document.set_metadata(
        Metadata::new()
            .title(title.to_owned())
            .creator("Abstract".into()),
    );
    for (index, commands) in layout.pages.into_iter().enumerate() {
        let mut page = document.start_page_with(PageSettings::from_wh(PAGE_W, PAGE_H).unwrap());
        let mut surface = page.surface();
        for command in commands {
            match command {
                Draw::Text { x, y, run } => {
                    surface.set_fill(Some(Fill {
                        paint: color(run.style.color.unwrap_or(INK)).into(),
                        ..Default::default()
                    }));
                    surface.set_stroke(None);
                    surface.draw_glyphs(
                        Point::from_xy(x, y),
                        &fonts.shape(&run.s, run.style),
                        fonts.face(run.style).font.clone(),
                        &run.s,
                        run.style.size.unwrap_or(BODY_PT) as f32,
                        false,
                    );
                }
                Draw::Line {
                    from,
                    to,
                    color: ink,
                } => {
                    let mut path = PathBuilder::new();
                    path.move_to(from.0, from.1);
                    path.line_to(to.0, to.1);
                    if let Some(path) = path.finish() {
                        surface.set_fill(None);
                        surface.set_stroke(Some(Stroke {
                            paint: color(ink).into(),
                            width: 0.5,
                            ..Default::default()
                        }));
                        surface.draw_path(&path);
                    }
                }
                Draw::Image { x, y, image } => {
                    surface.push_transform(&Transform::from_translate(x, y));
                    surface.draw_image(
                        image.pixels,
                        Size::from_wh(image.width as f32 * PT_MM, image.height as f32 * PT_MM)
                            .unwrap(),
                    );
                    surface.pop();
                }
            }
        }
        let number = (index + 1).to_string();
        let style = Style::new().with_font_size(8).with_color(FAINT);
        surface.set_stroke(None);
        surface.set_fill(Some(Fill {
            paint: color(FAINT).into(),
            ..Default::default()
        }));
        surface.draw_glyphs(
            Point::from_xy(
                (PAGE_W - fonts.width(&number, style)) / 2.,
                PAGE_H - FOOTER_Y as f32 * PT_MM,
            ),
            &fonts.shape(&number, style),
            fonts.face(style).font.clone(),
            &number,
            8.,
            false,
        );
        surface.finish();
        page.finish();
    }
    document
        .finish()
        .map_err(|error| io::Error::other(format!("{error:?}")))
}
