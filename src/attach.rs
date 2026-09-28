//! Attachments: images pasted or files dropped into a note. `import` does
//! blocking filesystem work and runs on the background executor; the rest are
//! pure path ↔ markdown helpers.

use std::io;
use std::path::{Component, Path, PathBuf};

/// Folder next to the note that receives pasted/dropped images.
pub const DIR: &str = "attachments";

#[derive(Clone)]
pub enum Incoming {
    /// Raw image bytes from the clipboard; `ext` without the dot.
    Image { bytes: Vec<u8>, ext: &'static str },
    /// A file or folder dropped/pasted from the OS.
    Path(PathBuf),
}

/// Store `items` for a note living in `note_dir` (inside the space `root`)
/// and return the markdown that references each one. `stamp` names pasted
/// images (`image-<stamp>.png`).
pub fn import(
    items: Vec<Incoming>,
    note_dir: &Path,
    root: &Path,
    stamp: &str,
) -> io::Result<Vec<String>> {
    let mut out = Vec::new();
    for item in items {
        out.push(match item {
            Incoming::Image { bytes, ext } => {
                let dir = note_dir.join(DIR);
                std::fs::create_dir_all(&dir)?;
                let path = unique_file(&dir, &format!("image-{stamp}"), ext);
                crate::store::write_atomic(&path, &bytes)?;
                format!("![]({})", encode(&relative(note_dir, &path)))
            }
            Incoming::Path(path) => reference(&path, note_dir, root)?,
        });
    }
    Ok(out)
}

fn reference(path: &Path, note_dir: &Path, root: &Path) -> io::Result<String> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = path
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let inside = path.starts_with(root);
    let is_file = path.is_file();
    if is_file && crate::md::is_image_path(&name) {
        let target = if inside {
            path.to_path_buf()
        } else {
            let dir = note_dir.join(DIR);
            std::fs::create_dir_all(&dir)?;
            let ext = path.extension().map(|x| x.to_string_lossy().into_owned());
            let copy = unique_file(&dir, &stem, ext.as_deref().unwrap_or("png"));
            std::fs::copy(path, &copy)?;
            copy
        };
        return Ok(format!(
            "![{}]({})",
            escape_alt(&stem),
            encode(&relative(note_dir, &target))
        ));
    }
    if is_file && inside && path.extension().is_some_and(|x| x == "md") {
        return Ok(format!("[[{stem}]]"));
    }
    let dest = if inside {
        encode(&relative(note_dir, path))
    } else {
        file_url(path)
    };
    Ok(format!("[{}]({dest})", escape_alt(&name)))
}

fn escape_alt(s: &str) -> String {
    s.replace('[', "\\[").replace(']', "\\]")
}

/// `dir/stem.ext`, or `dir/stem-2.ext`, … — the first that does not exist.
pub fn unique_file(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let mut n = 1;
    loop {
        let name = if n == 1 {
            format!("{stem}.{ext}")
        } else {
            format!("{stem}-{n}.{ext}")
        };
        let path = dir.join(name);
        if !path.exists() {
            return path;
        }
        n += 1;
    }
}

/// `to` relative to the directory `from`, `/`-separated. Falls back to the
/// absolute path when they share no root (different Windows drives).
pub fn relative(from: &Path, to: &Path) -> String {
    let a: Vec<Component> = from.components().collect();
    let b: Vec<Component> = to.components().collect();
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    if common == 0 {
        return to.to_string_lossy().replace('\\', "/");
    }
    let mut parts: Vec<String> = vec!["..".into(); a.len() - common];
    parts.extend(
        b[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    parts.join("/")
}

/// Percent-encode what would end or confuse a markdown link destination.
pub fn encode(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '%' => out.push_str("%25"),
            '(' => out.push_str("%28"),
            ')' => out.push_str("%29"),
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            '\\' => out.push('/'),
            c => out.push(c),
        }
    }
    out
}

fn file_url(path: &Path) -> String {
    let p = encode(&path.to_string_lossy());
    if p.starts_with('/') {
        format!("file://{p}")
    } else {
        format!("file:///{p}")
    }
}

/// Undo `<…>` wrapping and percent-encoding of a link destination.
pub fn decode(raw: &str) -> String {
    let raw = raw.trim();
    let raw = raw
        .strip_prefix('<')
        .and_then(|r| r.strip_suffix('>'))
        .unwrap_or(raw);
    let b = raw.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| raw.to_string())
}

/// Files an image reference may point at, most likely first. Remote URLs
/// yield nothing (the editor stays offline).
pub fn candidates(raw: &str, embed: bool, note_dir: &Path, root: &Path) -> Vec<PathBuf> {
    let src = decode(raw);
    if src.contains("://") && !src.starts_with("file://") || src.starts_with("data:") {
        return Vec::new();
    }
    if let Some(rest) = src.strip_prefix("file://") {
        let rest = if rest.get(2..3) == Some(":") {
            rest.trim_start_matches('/')
        } else {
            rest
        };
        return vec![PathBuf::from(rest)];
    }
    let p = Path::new(&src);
    if p.is_absolute() {
        return vec![p.to_path_buf()];
    }
    let mut out = vec![note_dir.join(p)];
    if embed {
        out.push(note_dir.join(DIR).join(p));
        out.push(root.join(p));
        out.push(root.join(DIR).join(p));
    }
    out
}

/// Seconds-resolution local-agnostic stamp for pasted image names.
pub fn stamp() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}{:03}", d.as_secs(), d.subsec_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths() {
        let r = Path::new("/v");
        assert_eq!(
            relative(r, Path::new("/v/attachments/a.png")),
            "attachments/a.png"
        );
        assert_eq!(
            relative(Path::new("/v/x/y"), Path::new("/v/z/a.png")),
            "../../z/a.png"
        );
    }

    #[test]
    fn encode_decode_round_trip() {
        let s = "my pics/a (1)%.png";
        assert_eq!(encode(s), "my%20pics/a%20%281%29%25.png");
        assert_eq!(decode(&encode(s)), s);
        assert_eq!(decode("<a b.png>"), "a b.png");
        assert_eq!(decode("100%"), "100%");
    }

    #[test]
    fn candidates_for_sources() {
        let (n, r) = (Path::new("/v/notes"), Path::new("/v"));
        assert_eq!(candidates("a%20b.png", false, n, r), [n.join("a b.png")]);
        assert_eq!(
            candidates("https://x/a.png", false, n, r),
            Vec::<PathBuf>::new()
        );
        assert_eq!(
            candidates("file:///tmp/a.png", false, n, r),
            [PathBuf::from("/tmp/a.png")]
        );
        assert_eq!(candidates("s.png", true, n, r).len(), 4);
    }

    #[test]
    fn import_stores_and_references() {
        let root = std::env::temp_dir().join(format!("abstract-attach-{}", stamp()));
        let notes = root.join("notes");
        let outside = root.with_extension("out");
        std::fs::create_dir_all(&notes).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("my cat.jpg"), b"jpg").unwrap();
        std::fs::write(outside.join("doc.pdf"), b"pdf").unwrap();
        std::fs::write(notes.join("Other.md"), b"# Other").unwrap();
        std::fs::write(root.join("in.png"), b"png").unwrap();

        let md = import(
            vec![
                Incoming::Image {
                    bytes: b"img".to_vec(),
                    ext: "png",
                },
                Incoming::Image {
                    bytes: b"img2".to_vec(),
                    ext: "png",
                },
                Incoming::Path(outside.join("my cat.jpg")),
                Incoming::Path(notes.join("Other.md")),
                Incoming::Path(root.join("in.png")),
                Incoming::Path(outside.join("doc.pdf")),
            ],
            &notes,
            &root,
            "1",
        )
        .unwrap();
        assert_eq!(md[0], "![](attachments/image-1.png)");
        assert_eq!(md[1], "![](attachments/image-1-2.png)");
        assert_eq!(md[2], "![my cat](attachments/my%20cat.jpg)");
        assert_eq!(md[3], "[[Other]]");
        assert_eq!(md[4], "![in](../in.png)");
        assert!(md[5].starts_with("[doc.pdf](file://") && md[5].ends_with("/doc.pdf)"));
        assert_eq!(
            std::fs::read(notes.join("attachments/image-1-2.png")).unwrap(),
            b"img2"
        );
        assert_eq!(
            std::fs::read(notes.join("attachments/my cat.jpg")).unwrap(),
            b"jpg"
        );
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }
}
