use super::*;
use crate::core::html::Context;

fn ctx() -> Context<'static> {
    Context {
        note_dir: None,
        root: None,
        tree: None,
    }
}

#[test]
fn export_emits_a_pdf() {
    let bytes = export("# Título\n\nparágrafo **bold** e `code`\n", "T", &ctx()).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    assert!(bytes.windows(5).any(|w| w == b"%%EOF"));
}

#[test]
fn weird_input_does_not_panic() {
    for text in [
        "", "# ", "\u{0}", "- \n", "[]()", "![](x)", "[^a]", "```", "> [!]",
    ] {
        let _ = export(text, "", &ctx()); // must not panic
    }
}

#[test]
fn big_note_exports() {
    let mut text = String::new();
    for i in 0..8000 {
        text.push_str(&format!(
            "## Seção {i}\n\nparágrafo com **bold** e ==mark==\n\n- item\n- item\n\n"
        ));
    }
    let bytes = export(&text, "big", &ctx()).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
}

/// The runs an `Emitter` produces for `text`, through the real
/// `inline()`/`special()` path.
fn runs(text: &str) -> Vec<StyledString> {
    let (mono, math) = (Family::Mono, Family::Math);
    let a = Analyzer::new().analyze(text);
    let c = ctx();
    let mut e = Emitter::new(text, &a, 0..text.len(), &c, mono, math);
    let mut flow = Flow::runs();
    e.inline(&mut flow, 0..text.len());
    flow.runs
}

fn joined(runs: &[StyledString]) -> String {
    runs.iter().map(|s| s.s.clone()).collect()
}

#[test]
fn math_converts_to_unicode() {
    let runs =
        runs("ver $\x5calpha^2 \x5cleq \x5cinfty$ ok\n\n$$\x5csum_i x_i \x5cto \x5cint f$$\n");
    let text = joined(&runs);
    assert!(text.contains("α² ≤ ∞"), "{text}");
    assert!(text.contains("∑ᵢ xᵢ → ∫ f"), "{text}");
    // Math runs print italic in the DejaVu face.
    assert!(
        runs.iter()
            .any(|r| r.style.is_italic() && r.s.contains('α')),
        "math run not italic"
    );
}

#[test]
fn unconvertible_math_shows_its_source() {
    // No superscript π exists, so `e^{i\pi}` degrades — the pieces that
    // still convert (\pi → π) must not drag the rest into TeX.
    let text = joined(&runs("$e^{i\x5cpi}$ e $\x5cfoo{n}$"));
    assert!(text.contains("e^iπ"), "{text}");
    assert!(text.contains("\\foo{n}"), "{text}");
}

#[test]
fn money_amounts_stay_literal() {
    let text = joined(&runs("R$ 10 e $ 20 no total"));
    assert!(text.contains("R$ 10 e $ 20"), "{text}");
}

#[test]
fn math_glyphs_reach_the_pdf() {
    // The writer embeds text as glyph ids; the ToUnicode CMap then lists
    // each used glyph as `<id> <codepoint>` — a converted char only
    // appears there if it really rendered (no .notdef tofu).
    let bytes = export(
        "ver $\x5calpha^2 \x5cleq \x5cinfty$\n\n$$\x5csum_i x_i \x5cto \x5cint f$$\n",
        "M",
        &ctx(),
    )
    .unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    let bytes = decoded_streams(&bytes);
    for cp in ["03b1", "00b2", "2264", "221e", "2211", "2192", "222b"] {
        let needle = format!("<{cp}>");
        assert!(
            bytes.windows(needle.len()).any(|w| w == needle.as_bytes()),
            "U+{cp} never reached a ToUnicode map"
        );
    }
}

fn decoded_streams(bytes: &[u8]) -> Vec<u8> {
    use std::io::Read;
    let mut out = Vec::new();
    for (index, _) in bytes
        .windows(7)
        .enumerate()
        .filter(|(_, b)| *b == b"stream\n")
    {
        let rest = &bytes[index + 7..];
        if let Some(end) = rest.windows(9).position(|b| b == b"endstream") {
            let mut decoder = flate2::read::ZlibDecoder::new(&rest[..end]);
            let mut decoded = Vec::new();
            if decoder.read_to_end(&mut decoded).is_ok() {
                out.extend(decoded);
            }
        }
    }
    out.extend_from_slice(bytes);
    out.make_ascii_lowercase();
    out
}
