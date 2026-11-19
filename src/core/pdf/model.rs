use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Color {
    Rgb(u8, u8, u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(super) enum Family {
    #[default]
    Sans,
    Mono,
    Math,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Style {
    pub color: Option<Color>,
    pub size: Option<u8>,
    pub family: Option<Family>,
    pub bold: bool,
    pub italic: bool,
}

impl Style {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_color(mut self, c: Color) -> Self {
        self.set_color(c);
        self
    }
    pub fn with_font_size(mut self, size: u8) -> Self {
        self.set_font_size(size);
        self
    }
    pub fn bold(mut self) -> Self {
        self.set_bold();
        self
    }
    pub fn italic(mut self) -> Self {
        self.set_italic();
        self
    }
    pub fn set_color(&mut self, c: Color) {
        self.color = Some(c);
    }
    pub fn set_font_family(&mut self, family: Family) {
        self.family = Some(family);
    }
    pub fn set_font_size(&mut self, size: u8) {
        self.size = Some(size);
    }
    pub fn set_bold(&mut self) {
        self.bold = true;
    }
    pub fn set_italic(&mut self) {
        self.italic = true;
    }
    #[cfg(test)]
    pub fn is_italic(&self) -> bool {
        self.italic
    }
    pub fn and(self, extra: Self) -> Self {
        Self {
            color: extra.color.or(self.color),
            size: extra.size.or(self.size),
            family: extra.family.or(self.family),
            bold: self.bold || extra.bold,
            italic: self.italic || extra.italic,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct StyledString {
    pub s: String,
    pub style: Style,
}
impl StyledString {
    pub fn new(s: impl Into<String>, style: Style) -> Self {
        Self { s: s.into(), style }
    }
}
impl From<&str> for StyledString {
    fn from(s: &str) -> Self {
        Self::new(s, Style::new())
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub(super) enum Alignment {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug)]
pub(super) struct Paragraph {
    pub runs: Vec<StyledString>,
    pub alignment: Alignment,
}
impl Paragraph {
    pub fn new(s: impl Into<StyledString>) -> Self {
        Self::from(vec![s.into()])
    }
    pub fn aligned(mut self, a: Alignment) -> Self {
        self.alignment = a;
        self
    }
}
impl From<Vec<StyledString>> for Paragraph {
    fn from(runs: Vec<StyledString>) -> Self {
        Self {
            runs,
            alignment: Alignment::Left,
        }
    }
}
impl From<Paragraph> for Block {
    fn from(p: Paragraph) -> Self {
        Self::Paragraph(p)
    }
}

#[derive(Clone, Default, Debug)]
pub(super) struct LinearLayout {
    pub blocks: Vec<Block>,
}
impl LinearLayout {
    pub fn vertical() -> Self {
        Self::default()
    }
    pub fn push(&mut self, el: impl Into<Block>) {
        self.blocks.push(el.into());
    }
}
impl From<LinearLayout> for Block {
    fn from(l: LinearLayout) -> Self {
        Self::Group(l.blocks)
    }
}

pub(super) struct Break(f64);
impl Break {
    pub fn new(height: f64) -> Self {
        Self(height)
    }
}
impl From<Break> for Block {
    fn from(b: Break) -> Self {
        Self::Gap(b.0)
    }
}

pub(super) struct Rule;
impl Rule {
    pub fn styled(self, style: Style) -> Block {
        Block::Rule(style.color.unwrap_or(super::LINE))
    }
}

impl Block {
    pub fn bar(inner: LinearLayout) -> Block {
        Block::Inset {
            inner: Box::new(inner.into()),
            margins: Margins::trbl(0., 0., 0., super::INDENT),
            frame: false,
            bar: true,
            color: super::LINE,
        }
    }
}

impl Block {
    pub fn bullet(inner: LinearLayout, label: String, style: Style) -> Block {
        Block::Bullet {
            inner: Box::new(inner.into()),
            label,
            style,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Margins {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}
impl Margins {
    pub fn all(v: f32) -> Self {
        Self::trbl(v as f64, v as f64, v as f64, v as f64)
    }
    pub fn trbl(top: f64, right: f64, bottom: f64, left: f64) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }
}
impl Block {
    pub fn padded(inner: impl Into<Block>, margins: Margins) -> Block {
        Block::Inset {
            inner: Box::new(inner.into()),
            margins,
            frame: false,
            bar: false,
            color: super::LINE,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct TableLayout {
    pub weights: Vec<usize>,
    pub rows: Vec<Vec<Block>>,
    pub color: Color,
}
impl TableLayout {
    pub fn new(weights: Vec<usize>) -> Self {
        Self {
            weights,
            rows: vec![],
            color: super::LINE,
        }
    }
    pub fn push_row(&mut self, row: Vec<Block>) {
        self.rows.push(row);
    }
    pub fn styled(mut self, style: Style) -> Self {
        self.color = style.color.unwrap_or(self.color);
        self
    }
}
impl From<TableLayout> for Block {
    fn from(t: TableLayout) -> Self {
        Self::Table(t)
    }
}

#[derive(Clone, Debug)]
pub(super) struct Image {
    pub pixels: krilla::image::Image,
    pub width: f64,
    pub height: f64,
    pub alignment: Alignment,
}
impl Image {
    pub fn from_path(path: &Path) -> std::io::Result<Self> {
        use std::io::Read;
        if !path.metadata()?.is_file() {
            return Err(std::io::Error::other("Not a regular image"));
        }
        let mut bytes = vec![];
        std::fs::File::open(path)?
            .take(crate::core::attach::MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > crate::core::attach::MAX_BYTES {
            return Err(std::io::Error::other("Image exceeds 32 MiB"));
        }
        let mut reader =
            image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let decoded = reader.decode().map_err(std::io::Error::other)?.into_rgba8();
        let (w, h) = decoded.dimensions();
        let pixels = krilla::image::Image::from_rgba8(decoded.into_raw(), w, h);
        Ok(Self {
            pixels,
            width: w as f64 * 25.4 / 300.,
            height: h as f64 * 25.4 / 300.,
            alignment: Alignment::Left,
        })
    }
    pub fn set_scale(&mut self, scale: Scale) {
        self.width *= scale.0;
        self.height *= scale.1;
    }
    pub fn set_alignment(&mut self, alignment: Alignment) {
        self.alignment = alignment;
    }
}
impl From<Image> for Block {
    fn from(image: Image) -> Self {
        Self::Image(image)
    }
}
pub(super) struct Scale(f64, f64);
impl Scale {
    pub fn new(x: f64, y: f64) -> Self {
        Self(x, y)
    }
}

#[derive(Clone, Debug)]
pub(super) enum Block {
    Paragraph(Paragraph),
    Group(Vec<Block>),
    Gap(f64),
    Rule(Color),
    Inset {
        inner: Box<Block>,
        margins: Margins,
        frame: bool,
        bar: bool,
        color: Color,
    },
    Bullet {
        inner: Box<Block>,
        label: String,
        style: Style,
    },
    Table(TableLayout),
    Image(Image),
}
impl Block {
    pub fn framed(mut self) -> Self {
        if let Self::Inset { frame, .. } = &mut self {
            *frame = true;
        }
        self
    }
    pub fn styled(mut self, style: Style) -> Self {
        if let Self::Inset { color, .. } = &mut self
            && let Some(c) = style.color
        {
            *color = c;
        }
        self
    }
}
