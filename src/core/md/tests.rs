use super::*;

fn shown(text: &str, cursor: usize) -> String {
    let a = Analyzer::new().analyze(text);
    let sel = cursor..cursor;
    a.lines
        .iter()
        .map(|(r, _)| {
            a.visible(r.clone(), &sel)
                .into_iter()
                .map(|v| &text[v])
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn emphasis_stays_inside_its_paragraph() {
    let t = "*a\n\nb*\n\n**c** or *d*\n";
    let a = Analyzer::new().analyze(t);
    let at = |s: &str| t.find(s).unwrap();
    assert_eq!(a.flags[at("a")] & ITALIC, 0);
    assert_eq!(a.flags[at("b")] & ITALIC, 0);
    assert_ne!(a.flags[at("c")] & BOLD, 0);
    assert_ne!(a.flags[at("d")] & ITALIC, 0);
}

#[test]
fn block_lead_leaves_a_blank_line() {
    assert_eq!(block_lead(""), "");
    assert_eq!(block_lead("alpha"), "\n\n");
    assert_eq!(block_lead("alpha\n"), "\n");
    assert_eq!(block_lead("alpha\n\n"), "");
}

#[test]
fn block_prefix_replaces_and_toggles() {
    assert_eq!(set_block_prefix("hello", "# "), "# hello");
    assert_eq!(set_block_prefix("## hello", "# "), "# hello");
    assert_eq!(set_block_prefix("# hello", "# "), "hello");
    assert_eq!(set_block_prefix("  - [x] a", "> "), "  > a");
    assert_eq!(set_block_prefix("3. a", "1. "), "a");
    assert_eq!(set_block_prefix("- a", "1. "), "1. a");
    assert_eq!(set_block_prefix("#tag", ""), "#tag");
    assert_eq!(set_block_prefix("### a", ""), "a");
}

#[test]
fn images_are_collected_and_concealed() {
    let t = "a ![cat](img/cat%20x.png) b\n![[shot.PNG]] and [[note]]\n![x](<a b.jpg>)";
    let a = Analyzer::new().analyze(t);
    let srcs: Vec<(&str, bool)> = a
        .images
        .iter()
        .map(|i| (&t[i.src.clone()], i.embed))
        .collect();
    assert_eq!(
        srcs,
        [
            ("img/cat%20x.png", false),
            ("shot.PNG", true),
            ("<a b.jpg>", false)
        ]
    );
    assert_eq!(&t[a.images[0].range.clone()], "![cat](img/cat%20x.png)");
    assert_eq!(&t[a.images[1].range.clone()], "![[shot.PNG]]");
    // Embeds are not wiki-links; the plain link stays one.
    assert_eq!(a.wiki_links.len(), 1);
    assert_eq!(
        shown(t, t.len()),
        "a cat b\nshot.PNG and note\n![x](<a b.jpg>)"
    );
    assert!(is_image_path("x.JPEG") && !is_image_path("x.md"));
}

#[test]
fn syntax_hidden_until_cursor_enters() {
    let t = "# Title\nx *i* **b** __u__ ~~s~~ `c` [l](http://a)";
    assert_eq!(shown(t, 0), "# Title\nx i b u s c l");
    // Cursor on the heading line reveals its marker only.
    assert_eq!(shown(t, 3), "# Title\nx i b u s c l");
    // Cursor on line 2 far from constructs: heading marker hidden.
    assert_eq!(shown(t, 9), "Title\nx i b u s c l");
    // Cursor touching `**b**` reveals exactly that construct.
    let b = t.find("**b").unwrap();
    assert_eq!(shown(t, b + 3), "Title\nx i **b** u s c l");
    let s = t.find("~~").unwrap();
    assert_eq!(shown(t, s + 5), "Title\nx i b u ~~s~~ c l");
}

#[test]
fn flags_and_kinds() {
    let t = "## H\n*i* **b** __u__ ~~s~~";
    let a = Analyzer::new().analyze(t);
    assert_eq!(a.lines[0].1, Kind::Heading(2));
    let at = |s: &str| a.flags[t.find(s).unwrap()];
    assert!(at("i*") & ITALIC != 0);
    assert!(at("b*") & BOLD != 0);
    assert!(at("u_") & UNDERLINE != 0 && at("u_") & BOLD == 0);
    assert!(at("s~") & STRIKE != 0);
}

#[test]
fn wiki_link_conceals_brackets() {
    let t = "see [[Foo|bar]] ok";
    let a = Analyzer::new().analyze(t);
    assert_eq!(a.wiki_links.len(), 1);
    let bar = t.find("bar").unwrap();
    // Cursor elsewhere: only the alias shows.
    assert_eq!(shown(t, 0), "see bar ok");
    assert!(a.flags[bar] & LINK != 0);
    // Cursor inside the link reveals the full syntax.
    assert_eq!(shown(t, bar), "see [[Foo|bar]] ok");
    // Plain target link shows the target.
    assert_eq!(shown("x [[Foo]] y", 0), "x Foo y");
}

#[test]
fn task_items_flag_marker_and_done_text() {
    let t = "- [ ] todo\n- [x] done";
    let a = Analyzer::new().analyze(t);
    assert_eq!(a.tasks.len(), 2);
    assert!(!a.tasks[0].checked);
    assert!(a.tasks[1].checked);
    let at = |s: &str| a.flags[t.find(s).unwrap()];
    assert!(at("[ ]") & TASK != 0 && at("[ ]") & MARK != 0);
    assert!(at("[x]") & TASK != 0);
    assert!(at("done") & DONE != 0);
    assert!(at("todo") & DONE == 0);
}

#[test]
fn list_items_track_depth_bullet_and_conceal() {
    let t = "- a\n  - b\n1. c\n- [x] d\n- [ ] e\n";
    let a = Analyzer::new().analyze(t);
    let bullets: Vec<_> = a.items.iter().map(|i| i.bullet).collect();
    assert_eq!(
        bullets,
        [
            Bullet::Dot,
            Bullet::Dot,
            Bullet::Ordered,
            Bullet::Task { checked: true },
            Bullet::Task { checked: false },
        ]
    );
    let depths: Vec<_> = a.items.iter().map(|i| i.depth).collect();
    assert_eq!(depths, [0, 1, 0, 0, 0]);
    // Concealed ranges cover the dash markers (with trailing spaces)
    // and the task boxes; the ordered marker stays visible.
    let hidden: Vec<_> = a.conceals.iter().map(|c| c.hidden.clone()).collect();
    assert!(hidden.contains(&(0..2)), "{hidden:?}");
    assert!(hidden.contains(&(6..8)), "{hidden:?}");
    let bx = t.find("[x]").unwrap();
    assert!(hidden.iter().any(|r| *r == (bx..bx + 4)), "{hidden:?}");
    let bu = t.find("[ ]").unwrap();
    assert!(hidden.iter().any(|r| *r == (bu..bu + 4)), "{hidden:?}");
    // `- [x] d` text still flagged DONE.
    let at = |s: &str| a.flags[t.find(s).unwrap()];
    assert!(at("[x]") & TASK != 0);
    assert!(at("d\n") & DONE != 0);
}

#[test]
fn highlight_and_comments_conceal_delimiters() {
    let t = "a ==hi== b %%note%% c `==x==` == d ==\n=== e";
    let a = Analyzer::new().analyze(t);
    let at = |s: &str| a.flags[t.find(s).unwrap()];
    assert!(at("hi") & HIGHLIGHT != 0);
    assert!(at("note") & MUTED != 0 && at("note") & ITALIC != 0);
    assert!(at("x==") & HIGHLIGHT == 0);
    assert!(at(" d") & HIGHLIGHT == 0);
    assert_eq!(shown(t, t.len()), "a hi b note c ==x== == d ==\n=== e");
    let m = "%%\nsecret\n%% after";
    let a = Analyzer::new().analyze(m);
    assert!(a.flags[m.find("secret").unwrap()] & MUTED != 0);
    assert!(a.flags[m.find("after").unwrap()] & MUTED == 0);
}

#[test]
fn highlight_tints() {
    let t = "a =={red}hot== =={blue}cold== ==plain== =={bogus}weird==\n";
    let a = Analyzer::new().analyze(t);
    let at = |s: &str| t.find(s).unwrap();
    let spans: Vec<_> = a
        .marks
        .iter()
        .map(|m| (&t[m.range.clone()], m.tint))
        .collect();
    assert_eq!(
        spans,
        [
            ("=={red}hot==", Tint::Red),
            ("=={blue}cold==", Tint::Blue),
            ("==plain==", Tint::Yellow),
            ("=={bogus}weird==", Tint::Yellow),
        ]
    );
    // The content is highlighted; the {colour} prefix conceals with the `==`.
    assert!(a.flags[at("hot")] & HIGHLIGHT != 0);
    assert!(a.flags[at("{red}")] & MARK != 0);
    assert!(a.flags[at("{red}")] & HIGHLIGHT == 0);
    // Unknown names stay literal inside a plain highlight.
    assert!(a.flags[at("{bogus}")] & HIGHLIGHT != 0);
    assert_eq!(a.tint_at(at("hot")), Some(Tint::Red));
    assert_eq!(a.tint_at(at("cold")), Some(Tint::Blue));
    assert_eq!(a.tint_at(at("plain")), Some(Tint::Yellow));
    assert_eq!(a.tint_at(at("a")), None);
    // Caret outside: only content (and a literal unknown prefix) shows.
    assert_eq!(shown(t, t.len()), "a hot cold plain {bogus}weird\n");
    // Caret inside the mark reveals `=={red}…==` whole.
    assert_eq!(
        shown(t, at("hot")),
        "a =={red}hot== cold plain {bogus}weird\n"
    );
}

#[test]
fn highlight_tint_edge_cases() {
    let t = "=={RED}x== =={green}== =={red}{blue}z== =={cyan}w==\n=={purple}a\nb==";
    let a = Analyzer::new().analyze(t);
    let at = |s: &str| t.find(s).unwrap();
    // Case-insensitive names; `{red}{blue}z` is red with a literal `{blue}`.
    let tints: Vec<_> = a.marks.iter().map(|m| m.tint).collect();
    assert_eq!(tints, [Tint::Red, Tint::Yellow, Tint::Red, Tint::Yellow]);
    // A bare `=={green}==` keeps the prefix literal; so does unknown `{cyan}`.
    assert!(a.flags[at("{green}")] & HIGHLIGHT != 0);
    assert!(a.flags[at("{cyan}")] & HIGHLIGHT != 0);
    assert!(a.flags[at("{blue}z")] & HIGHLIGHT != 0);
    // Marks do not span lines.
    assert_eq!(a.marks.len(), 4);
    // Unclosed and code-span `==…==` stay literal.
    let t = "=={red}open `=={blue}in code==`\n";
    let a = Analyzer::new().analyze(t);
    assert!(a.marks.is_empty());
    assert_eq!(shown(t, t.len()), "=={red}open =={blue}in code==\n");
}

#[test]
fn callouts_take_tone_and_title() {
    let t = "> [!WARNING] Careful\n> body\n\n> [!tip]\n> x\n\n> plain";
    let a = Analyzer::new().analyze(t);
    assert_eq!(
        a.callouts,
        [
            Callout {
                lines: 0..2,
                tone: Tone::Warning
            },
            Callout {
                lines: 3..5,
                tone: Tone::Tip
            }
        ]
    );
    assert!(a.flags[t.find("Careful").unwrap()] & CALLOUT != 0);
    assert!(a.flags[t.find("tip").unwrap()] & CALLOUT != 0);
    assert_eq!(shown(t, 0), "> [!WARNING] Careful\nbody\n\ntip\nx\n\nplain");
}

#[test]
fn callout_toggles() {
    assert_eq!(toggle_callout("a\n- b"), "> [!note] a\n> - b");
    assert_eq!(toggle_callout("> q"), "> [!note] q");
    assert_eq!(toggle_callout("> [!note] a\n> - b"), "a\n- b");
    assert_eq!(toggle_callout("> [!tip]"), "");
}

#[test]
fn pipe_tables_are_table_lines() {
    let text = "intro\n\n| a | b\\|c |\n| - | - |\n| 1 | 2 |\n\nafter";
    let a = Analyzer::new().analyze(text);
    assert_eq!(a.tables, vec![2..5_usize]);
    let kinds: Vec<Kind> = a.lines.iter().map(|l| l.1).collect();
    assert_eq!(kinds[2..5], [Kind::Table; 3]);
    assert_eq!(kinds[6], Kind::Body);
    let header = a.lines[2].0.start;
    assert_ne!(a.flags[header] & MUTED, 0);
    assert_ne!(a.flags[header + 2] & BOLD, 0);
    let escaped = header + text[header..].find("\\|").unwrap() + 1;
    assert_eq!(a.flags[escaped] & MUTED, 0);
    assert!(shown(text, 0).contains("| b\\|c |"));
}

#[test]
fn tables_keep_rows_of_empty_cells() {
    for text in [
        "| a | b |\n| - | - |\n| 1 | 2 |\n|   |   |\n",
        "| a | b |\n| - | - |\n| 1 | 2 |\n| | |\n\nafter",
    ] {
        let a = Analyzer::new().analyze(text);
        assert_eq!(a.tables, vec![0..4_usize], "{text:?}");
        assert_eq!(a.lines[3].1, Kind::Table);
        assert_eq!(a.lines.last().unwrap().1, Kind::Body);
    }
}

#[test]
fn footnotes_render_as_bracketed_links() {
    let t = "Hi[^1] `[^2]`.\n\n[^1]: Source.";
    let a = Analyzer::new().analyze(t);
    assert_eq!(a.footnotes.len(), 2);
    assert!(a.footnotes[1].def);
    assert_eq!(shown(t, t.len()), "Hi[1] [^2].\n\n[1]: Source.");
    let label = t.find('1').unwrap();
    assert_ne!(a.flags[label] & LINK, 0);
    let body = t.find("Source").unwrap();
    assert_eq!(a.flags[body] & MUTED, 0);
    // The caret on the reference reveals its `^`.
    assert!(shown(t, 3).starts_with("Hi[^1]"));
}

#[test]
fn heading_typed_at_eof() {
    let a = Analyzer::new().analyze("# A");
    assert_eq!(a.lines[0].1, Kind::Heading(1));
}

#[test]
fn list_continuation() {
    assert_eq!(list_prefix("- item"), Some((2, "- ".into())));
    assert_eq!(list_prefix("  9. x"), Some((5, "  10. ".into())));
    assert_eq!(list_prefix("- [x] done"), Some((6, "- [ ] ".into())));
    assert_eq!(list_prefix("plain"), None);
}

#[test]
fn math_inline_conceals_dollars() {
    let t = "soma $x + 1$ fim";
    let a = Analyzer::new().analyze(t);
    assert_eq!(a.maths.len(), 1);
    assert!(!a.maths[0].block);
    assert_eq!(&t[a.maths[0].content.clone()], "x + 1");
    assert_ne!(a.flags[t.find('$').unwrap()] & MARK, 0);
    // Hidden until the caret touches the span, exactly like `**b**`.
    assert_eq!(shown(t, 0), "soma x + 1 fim");
    assert_eq!(shown(t, t.find('x').unwrap()), "soma $x + 1$ fim");
    // A bare single-`$` pair works too.
    let t = "a $x$ b";
    assert_eq!(Analyzer::new().analyze(t).maths.len(), 1);
}

#[test]
fn math_money_and_escapes_are_literal() {
    for t in [
        "R$ 10 e $ 20",
        "$10",
        "$x$5",
        "a\\$b",
        "um $x",
        "$ x$",
        "custa $$ 100",
    ] {
        let a = Analyzer::new().analyze(t);
        assert!(a.maths.is_empty(), "{t:?}");
    }
}

#[test]
fn math_skips_code_and_tables() {
    let t = "`$x$` e $y$\n\n```\n$z$\n```\n| a | b |\n| - | - |\n| $w$ | q |\n";
    let a = Analyzer::new().analyze(t);
    assert_eq!(a.maths.len(), 1, "{:?}", a.maths);
    assert_eq!(&t[a.maths[0].content.clone()], "y");
}

#[test]
fn math_block_multiline() {
    let t = "antes\n\n$$\n\\sum_i x\n$$\n\ndepois\n";
    let a = Analyzer::new().analyze(t);
    assert_eq!(a.maths.len(), 1);
    assert!(a.maths[0].block);
    assert_eq!(&t[a.maths[0].content.clone()], "\n\\sum_i x\n");
    assert_eq!(shown(t, 0), "antes\n\n\n\\sum_i x\n\n\ndepois\n");
    // `$$` alone on a line also blocks; mid-paragraph it stays inline.
    assert!(Analyzer::new().analyze("$$x^2$$").maths[0].block);
    assert!(!Analyzer::new().analyze("v $$x$$ w").maths[0].block);
}

fn block_of(t: &str, cursor: usize) -> String {
    let a = Analyzer::new().analyze(t);
    t[a.focus_block(t, cursor)].to_string()
}

#[test]
fn focus_block_paragraph() {
    let t = "one\ntwo\n\nthree\nfour\n\nfive";
    let at = |s: &str| t.find(s).unwrap();
    // Inside a paragraph the whole run lights, first line included.
    assert_eq!(block_of(t, at("four")), "three\nfour");
    assert_eq!(block_of(t, at("three")), "three\nfour");
    // First and last lines of the document bound the run too.
    assert_eq!(block_of(t, 0), "one\ntwo");
    assert_eq!(block_of(t, t.len()), "five");
    // On a blank line only that line is the block (all text dims).
    assert_eq!(block_of(t, at("\n\nthree") + 1), "");
}

#[test]
fn focus_block_list_item() {
    let t = "- one\n- two\n  cont\n- three\n\npara";
    let at = |s: &str| t.find(s).unwrap();
    // Each item is its own unit; a continuation line belongs to its item.
    assert_eq!(block_of(t, at("two")), "- two\n  cont");
    assert_eq!(block_of(t, at("cont")), "- two\n  cont");
    assert_eq!(block_of(t, at("one")), "- one");
    assert_eq!(block_of(t, at("three")), "- three");
    assert_eq!(block_of(t, at("para")), "para");
}

#[test]
fn focus_block_heading() {
    let t = "# Title\n\nlead\n\n## Sub\nbody";
    let at = |s: &str| t.find(s).unwrap();
    assert_eq!(block_of(t, at("Title")), "# Title");
    assert_eq!(block_of(t, at("Sub")), "## Sub\nbody");
    assert_eq!(block_of(t, at("body")), "## Sub\nbody");
    assert_eq!(block_of(t, at("lead")), "lead");
}

#[test]
fn focus_block_empty_document() {
    assert_eq!(block_of("", 0), "");
    assert_eq!(block_of("   ", 0), "   ");
    let a = Analyzer::new().analyze("");
    assert_eq!(a.focus_block("", 0), 0..0);
}
