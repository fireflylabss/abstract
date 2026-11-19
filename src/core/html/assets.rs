use std::io::Read;

use super::Context;
use crate::core::attach;

pub(super) fn image_source(raw: &str, embed: bool, ctx: &Context) -> Option<String> {
    let dir = ctx.note_dir?;
    for path in attach::candidates(raw, embed, dir, ctx.root.unwrap_or(dir)) {
        let mime = match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            "bmp" => "image/bmp",
            _ => continue,
        };
        let Ok(meta) = path.metadata() else {
            continue;
        };
        if !meta.is_file() || meta.len() > attach::MAX_BYTES as u64 {
            continue;
        }
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        let mut bytes = Vec::new();
        if file
            .take(attach::MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .is_err()
            || bytes.len() > attach::MAX_BYTES
        {
            continue;
        }
        return Some(format!("data:{mime};base64,{}", base64(&bytes)));
    }
    None
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for block in bytes.chunks(3) {
        let a = block[0] as usize;
        let b = block.get(1).copied().unwrap_or(0) as usize;
        let c = block.get(2).copied().unwrap_or(0) as usize;
        out.push(ABC[a >> 2] as char);
        out.push(ABC[(a & 3) << 4 | b >> 4] as char);
        out.push(if block.len() > 1 {
            ABC[(b & 15) << 2 | c >> 6] as char
        } else {
            '='
        });
        out.push(if block.len() > 2 {
            ABC[c & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
#[path = "assets_tests.rs"]
mod tests;
