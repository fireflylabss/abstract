use super::Dir;
use crate::io::export::{Exports, Format, Job, Snapshot};
use std::path::Path;
use std::time::Duration;

fn submit(
    root: &Path,
    text: Option<&str>,
    format: Format,
    dest: &Path,
) -> Result<std::path::PathBuf, String> {
    let worker = Exports::new();
    worker
        .submit(Job {
            source: Snapshot {
                path: root.join("Note.md"),
                text: text.map(str::to_owned),
                root: root.to_owned(),
                tree: vec![],
            },
            destination: dest.to_owned(),
            format,
        })
        .unwrap();
    worker
        .replies
        .recv_timeout(Duration::from_secs(30))
        .unwrap()
}

#[test]
fn exports_empty_and_buffer_only_notes_to_both_formats() {
    let dir = Dir::new();
    for format in [Format::Html, Format::Pdf] {
        for text in ["", "# Buffer only\n\nnot on disk"] {
            let dest = dir.0.join(format!("output.{}", format.extension()));
            submit(&dir.0, Some(text), format, &dest).unwrap();
            let bytes = std::fs::read(dest).unwrap();
            assert!(bytes.starts_with(if format == Format::Html {
                b"<!doctype html>"
            } else {
                b"%PDF"
            }));
            if format == Format::Html {
                assert!(String::from_utf8(bytes).unwrap().contains("<title>"));
            }
        }
    }
}

#[test]
fn export_reads_closed_notes_off_thread_and_reports_read_and_write_errors() {
    let dir = Dir::new();
    let path = dir.0.join("Note.md");
    let dest = dir.0.join("out.html");
    assert!(submit(&dir.0, None, Format::Html, &dest).is_err());
    assert!(!dest.exists());
    std::fs::write(&path, b"\xff").unwrap();
    assert!(submit(&dir.0, None, Format::Html, &dest).is_err());
    std::fs::write(&path, "# Closed note").unwrap();
    submit(&dir.0, None, Format::Html, &dest).unwrap();
    assert!(
        std::fs::read_to_string(&dest)
            .unwrap()
            .contains("Closed note")
    );
    assert!(submit(&dir.0, Some("# Buffer"), Format::Html, &path).is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "# Closed note");
    std::fs::remove_file(&dest).unwrap();
    std::fs::create_dir(&dest).unwrap();
    assert!(submit(&dir.0, Some("# Buffer"), Format::Html, &dest).is_err());
    assert_eq!(std::fs::read_dir(&dest).unwrap().count(), 0);
}

#[test]
fn sample_with_math_code_table_and_image_is_self_contained() {
    let dir = Dir::new();
    let image = image::RgbImage::from_pixel(80, 40, image::Rgb([50, 150, 210]));
    image.save(dir.0.join("sample.png")).unwrap();
    let source = "# Sample\n\n**Bold** and $x^2$.\n\n```rust\nlet x = 1;\n```\n\n| Key | Value |\n| --- | --- |\n| A | B |\n\n![Sample](sample.png)\n";
    let html = dir.0.join("sample.html");
    submit(&dir.0, Some(source), Format::Html, &html).unwrap();
    let html = std::fs::read_to_string(html).unwrap();
    for expected in [
        "<h1>",
        "<pre>",
        "<table>",
        "<math",
        "data:image/png;base64,iVBOR",
    ] {
        assert!(html.contains(expected), "missing {expected}");
    }
    let pdf = dir.0.join("sample.pdf");
    submit(&dir.0, Some(source), Format::Pdf, &pdf).unwrap();
    let pdf = std::fs::read(pdf).unwrap();
    assert!(pdf.windows(6).any(|w| w == b"/Image"));
}
