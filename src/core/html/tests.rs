use super::*;
use crate::core::links::parse;

fn ctx() -> Context<'static> {
    Context {
        note_dir: None,
        root: None,
        tree: None,
    }
}

fn html(text: &str) -> String {
    let a = Analyzer::new().analyze(text);
    fragment(text, &a, 0..text.len(), &ctx())
}

#[test]
fn headings_and_inline_styles() {
    let out = html("# Title\n\nA **bold** *it* __u__ ~~del~~ `co` ==mk==.\n");
    assert!(out.contains("<h1>Title</h1>"));
    assert!(out.contains("<strong>bold</strong>"));
    assert!(out.contains("<em>it</em>"));
    assert!(out.contains("<u>u</u>"));
    assert!(out.contains("<del>del</del>"));
    assert!(out.contains("<code>co</code>"));
    assert!(out.contains("<mark>mk</mark>"));
}

#[test]
fn tinted_marks_export_their_colour() {
    let out = html("=={red}hot== =={blue}cold== ==plain== =={bogus}x==\n");
    assert!(out.contains("<mark style=\"background:#ffd7d5\">hot</mark>"));
    assert!(out.contains("<mark style=\"background:#d3e3fd\">cold</mark>"));
    // Bare and unknown-colour marks export a plain `<mark>`.
    assert!(out.contains("<mark>plain</mark>"));
    assert!(out.contains("<mark>{bogus}x</mark>"));
}

#[test]
fn escapes_text() {
    let out = html("a < b & c > d \"e\"\n");
    assert!(out.contains("a &lt; b &amp; c &gt; d \"e\""));
}

#[test]
fn comments_are_omitted() {
    let out = html("keep %%private%% this\n");
    assert_eq!(out, "<p>keep  this</p>\n");
    let out = html("%%whole line%%\n\nshown\n");
    assert!(!out.contains("whole line"));
    assert!(out.contains("shown"));
    // Inside code spans `%%` is literal.
    let out = html("`%%x%%`\n");
    assert!(out.contains("<code>%%x%%</code>"));
}

#[test]
fn links_render_with_destinations() {
    let out = html("[site](https://x.dev) <https://y.dev>\n\n[ref][id]\n\n[id]: /docs\n");
    assert!(out.contains("<a href=\"https://x.dev\">site</a>"));
    assert!(out.contains("<a href=\"https://y.dev\">https://y.dev</a>"));
    assert!(out.contains("<a href=\"/docs\">ref</a>"));
    // The definition itself stays hidden.
    assert!(!out.contains("[id]:"));
}

#[test]
fn wiki_links() {
    let out = html("see [[Target|the target]] and [[Missing]]\n");
    assert!(out.contains("<span class=\"wikilink\">the target</span>"));
    assert!(out.contains("<span class=\"wikilink\">Missing</span>"));
}

#[test]
fn tasks_and_lists() {
    let out = html("- [ ] todo\n- [x] done\n\n1. one\n2. two\n");
    assert!(out.contains("<input type=\"checkbox\" disabled> todo"));
    assert!(out.contains("<input type=\"checkbox\" checked disabled> done"));
    assert!(out.contains("<ol>\n<li>one</li>\n<li>two</li>\n</ol>"));
}

#[test]
fn code_blocks() {
    let out = html("```rust\nlet a = 1 < 2;\n```\n");
    assert!(out.contains("<pre><code class=\"language-rust\">let a = 1 &lt; 2;\n</code></pre>"));
}

#[test]
fn tables() {
    let out = html("| a | b |\n|:-:|--:|\n| 1 | 2 |\n");
    assert!(out.contains("<th style=\"text-align:center\"><strong>a</strong></th>"));
    assert!(out.contains("<th style=\"text-align:right\"><strong>b</strong></th>"));
    assert!(out.contains("<td style=\"text-align:center\">1</td>"));
}

#[test]
fn callouts() {
    let out = html("> [!warning] Careful\n> body text\n");
    assert!(out.contains("class=\"callout callout-warning\""));
    assert!(out.contains("<p class=\"callout-title\"><strong>Careful</strong></p>"));
    assert!(out.contains("<p>body text</p>"));
    assert!(out.contains("body text"));
    // A plain quote stays a blockquote.
    let out = html("> just a quote\n");
    assert!(out.contains("<blockquote>"));
}

#[test]
fn footnotes() {
    // Adjacent defs: each `<li>` ends at its own body.
    let out = html("Text[^1] and[^note].\n\n[^1]: First footnote.\n[^note]: Second **bold**.\n");
    assert!(
        out.contains("<sup class=\"footnote-ref\" id=\"fnref-1\"><a href=\"#fn-1\">1</a></sup>")
    );
    assert!(out.contains("class=\"footnotes\""));
    assert!(out.contains("<li id=\"fn-1\"><sup>1</sup> First footnote. <a"));
    assert!(!out.contains("First footnote.\n Second"));
    assert!(out.contains("<li id=\"fn-note\"><sup>note</sup> Second <strong>bold</strong>."));
    assert!(out.contains("href=\"#fnref-note\""));
}

#[test]
fn images() {
    let out = html("![cat](pic%20one.png)\n");
    assert!(out.contains("<img src=\"pic%20one.png\" alt=\"cat\">"));
}

#[test]
fn selection_clips() {
    let text = "first\n\nsecond para\n\nthird\n";
    let a = Analyzer::new().analyze(text);
    let sel = text.find("second").unwrap()..text.find("para").unwrap() + 4;
    let out = fragment(text, &a, sel.clone(), &ctx());
    assert_eq!(out, "<p>second para</p>\n");
    // A selection inside a paragraph clips to it.
    let sel = text.find("eco").unwrap()..text.find("eco").unwrap() + 3;
    let out = fragment(text, &a, sel, &ctx());
    assert_eq!(out, "<p>eco</p>\n");
}

#[test]
fn plain_text_fallback() {
    let text = "**bold** `code` %%secret%% [[Link]]\n";
    let a = Analyzer::new().analyze(text);
    let p = plain(text, &a, 0..text.len());
    assert_eq!(p, "bold code  Link");
}

#[test]
fn export_document() {
    let out = export("# T\n\nbody %%hidden%%\n", "T", &ctx());
    assert!(out.starts_with("<!doctype html>"));
    assert!(out.contains("<title>T</title>"));
    assert!(out.contains("<style>"));
    assert!(out.contains("<h1>T</h1>"));
    assert!(!out.contains("hidden"));
}

#[test]
fn wiki_link_targets_parse() {
    let links = parse("[[A]] [[B|x]]", |_| false);
    assert_eq!(links.len(), 2);
}
