//! Attachments: images pasted or files dropped into a note. `import` does
//! blocking filesystem work and runs on the background executor; the rest are
//! pure path ↔ markdown helpers.

use std::io;
use std::io::Write;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

use crate::core::md::ImageRef;

/// Folder next to the note that receives pasted/dropped images.
pub const DIR: &str = "attachments";
pub const MAX_BYTES: usize = 32 * 1024 * 1024;

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
                if bytes.len() > MAX_BYTES {
                    return Err(io::Error::other("Image exceeds 32 MiB"));
                }
                let path = write_unique(&dir, &format!("image-{stamp}"), ext, |file| {
                    file.write_all(&bytes)
                })?;
                format!("![]({})", encode(&relative(note_dir, &path)))
            }
            Incoming::Path(path) => reference(&path, note_dir, root)?,
        });
    }
    Ok(out)
}

fn reference(path: &Path, note_dir: &Path, root: &Path) -> io::Result<String> {
    let metadata = path.metadata()?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = path
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let inside = path.starts_with(root);
    let is_file = metadata.is_file();
    if is_file && crate::core::md::is_image_path(&name) {
        let target = if inside {
            path.to_path_buf()
        } else {
            let dir = note_dir.join(DIR);
            std::fs::create_dir_all(&dir)?;
            let ext = path.extension().map(|x| x.to_string_lossy().into_owned());
            let mut input = std::fs::File::open(path)?;
            if input.metadata()?.len() > MAX_BYTES as u64 {
                return Err(io::Error::other("Image exceeds 32 MiB"));
            }
            write_unique(&dir, &stem, ext.as_deref().unwrap_or("png"), |output| {
                use std::io::Read;
                let copied = std::io::copy(
                    &mut Read::by_ref(&mut input).take(MAX_BYTES as u64 + 1),
                    output,
                )?;
                if copied > MAX_BYTES as u64 {
                    return Err(io::Error::other("Image exceeds 32 MiB"));
                }
                Ok(())
            })?
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

fn write_unique(
    dir: &Path,
    stem: &str,
    ext: &str,
    mut write: impl FnMut(&mut std::fs::File) -> io::Result<()>,
) -> io::Result<PathBuf> {
    for n in 1.. {
        let name = if n == 1 {
            format!("{stem}.{ext}")
        } else {
            format!("{stem}-{n}.{ext}")
        };
        let path = dir.join(name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                let result = write(&mut file).and_then(|()| file.sync_all());
                drop(file);
                if let Err(error) = result {
                    let _ = std::fs::remove_file(&path);
                    return Err(error);
                }
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    unreachable!()
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

/// Edits, last first, that keep relative `![](…)` sources pointing at the
/// same files once a note moves from `old_dir` to `new_dir`. `![[…]]` embeds
/// also resolve against the space root and are left alone.
pub fn rebase(
    text: &str,
    images: &[ImageRef],
    old_dir: &Path,
    new_dir: &Path,
) -> Vec<(Range<usize>, String)> {
    let mut out = Vec::new();
    for im in images.iter().rev().filter(|im| !im.embed) {
        let src = decode(&text[im.src.clone()]);
        if src.is_empty() || src.contains(':') || Path::new(&src).is_absolute() {
            continue;
        }
        let to = encode(&relative(new_dir, &normalize(&old_dir.join(&src))));
        if to != text[im.src.clone()] {
            out.push((im.src.clone(), to));
        }
    }
    out
}

/// `.` and `..` folded away lexically.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
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
mod tests;
