//! Honest LaTeX approximation for the live editor: common commands map to
//! their Unicode glyph (`\alpha` → α), `^`/`_` become super/subscript when
//! every char in the atom has a glyph, `\frac`/`√` take their arguments
//! literally, and anything unknown stays in the source verbatim — math text
//! is never swallowed.

use std::ops::Range;

/// A piece of math source that displays as `text` instead of verbatim.
#[derive(Clone, Debug)]
pub struct Sub {
    /// Byte range in the string `subs` was called on.
    pub range: Range<usize>,
    pub text: String,
}

/// `src` with every substitution applied — the approximation of a nested
/// argument (`\frac{a^2}{b}` → `(a²)/(b)`).
pub fn approx(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut at = 0;
    for s in subs(src) {
        out.push_str(&src[at..s.range.start]);
        out.push_str(&s.text);
        at = s.range.end;
    }
    out.push_str(&src[at..]);
    out
}

/// The substituted pieces of `src`, in order; spans between them render
/// literally.
pub fn subs(src: &str) -> Vec<Sub> {
    let b = src.as_bytes();
    let mut out: Vec<Sub> = Vec::new();
    // `out` indices of `{` subs with no `}` yet; unclosed braces revert to
    // literal text at the end.
    let mut braces: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\\' => i = command(src, i, &mut out),
            b'^' | b'_' => i = script(src, i, &mut out),
            b'{' => {
                braces.push(out.len());
                out.push(Sub {
                    range: i..i + 1,
                    text: String::new(),
                });
                i += 1;
            }
            b'}' => {
                if braces.pop().is_some() {
                    out.push(Sub {
                        range: i..i + 1,
                        text: String::new(),
                    });
                }
                i += 1;
            }
            b'~' => {
                out.push(Sub {
                    range: i..i + 1,
                    text: "\u{a0}".into(),
                });
                i += 1;
            }
            _ => i += 1,
        }
    }
    for ix in braces {
        out[ix].text = "{".into();
    }
    out
}

/// A `\command` starting at `i`; returns where scanning resumes. Commands
/// whose arguments fail to parse consume their parsed groups verbatim, so a
/// malformed `\frac{a}` stays `\frac{a}` rather than losing its braces.
fn command(src: &str, i: usize, out: &mut Vec<Sub>) -> usize {
    let b = src.as_bytes();
    let mut e = i + 1;
    while e < b.len() && b[e].is_ascii_alphabetic() {
        e += 1;
    }
    if e == i + 1 {
        // Control symbol: a single non-letter (`\{`, `\,`, `\\`).
        e = (i + 2).min(b.len());
    }
    let name = &src[i + 1..e];
    if name.is_empty() {
        return e;
    }
    match name {
        "frac" | "dfrac" | "tfrac" => {
            let Ok((a, e1)) = arg(src, e) else {
                return e;
            };
            let Ok((b2, e2)) = arg(src, e1) else {
                return e1;
            };
            out.push(Sub {
                range: i..e2,
                text: format!("({})/({})", approx(&src[a]), approx(&src[b2])),
            });
            e2
        }
        "sqrt" => {
            // Optional `[n]` root index before the argument.
            let mut e1 = e;
            while e1 < b.len() && b[e1] == b' ' {
                e1 += 1;
            }
            let mut root = None;
            if b.get(e1) == Some(&b'[')
                && let Some(close) = src[e1..].find(']')
            {
                root = Some(e1 + 1..e1 + close);
                e1 += close + 1;
            }
            let Ok((a, e2)) = arg(src, e1) else {
                return e1;
            };
            let text = match root {
                Some(r) => match super_all(&src[r.clone()], true) {
                    Some(sup) => format!("{sup}√({})", approx(&src[a])),
                    None => format!("√[{}]({})", approx(&src[r]), approx(&src[a])),
                },
                None => format!("√({})", approx(&src[a])),
            };
            out.push(Sub { range: i..e2, text });
            e2
        }
        "mathbb" => {
            let Ok((a, e1)) = arg(src, e) else {
                return e;
            };
            match double_struck(&src[a]) {
                Some(text) => {
                    out.push(Sub { range: i..e1, text });
                    e1
                }
                None => e1,
            }
        }
        "text" | "mathrm" | "mathbf" | "mathit" | "mathsf" | "mathtt" | "operatorname"
        | "boldsymbol" | "mathop" | "mathbin" | "mathrel" | "mathinner" => {
            let Ok((a, e1)) = arg(src, e) else {
                return e;
            };
            out.push(Sub {
                range: i..e1,
                text: approx(&src[a]),
            });
            e1
        }
        "vec" | "hat" | "widehat" | "bar" | "overline" | "underline" | "dot" | "ddot" | "tilde"
        | "widetilde" | "check" | "breve" | "acute" | "grave" | "mathring" => {
            let Ok((a, e1)) = arg(src, e) else {
                return e;
            };
            let mark = match name {
                "vec" => '\u{20d7}',
                "hat" | "widehat" => '\u{0302}',
                "bar" => '\u{0304}',
                "overline" => '\u{0305}',
                "underline" => '\u{0332}',
                "dot" => '\u{0307}',
                "ddot" => '\u{0308}',
                "tilde" | "widetilde" => '\u{0303}',
                "check" => '\u{030c}',
                "breve" => '\u{0306}',
                "acute" => '\u{0301}',
                "grave" => '\u{0300}',
                _ => '\u{030a}',
            };
            out.push(Sub {
                range: i..e1,
                text: format!("{}{}", approx(&src[a]), mark),
            });
            e1
        }
        "left" | "right" | "middle" | "big" | "Big" | "bigg" | "Bigg" | "bigl" | "bigr"
        | "Bigl" | "Bigr" | "biggl" | "biggr" | "Biggl" | "Biggr" | "bigm" | "Bigm" | "biggm"
        | "Biggm" => {
            // Sizing/delimiter commands: the following atom renders bare.
            let Ok((a, e1)) = arg(src, e) else {
                return e;
            };
            let text = if &src[a.clone()] == "." {
                String::new()
            } else {
                approx(&src[a])
            };
            out.push(Sub { range: i..e1, text });
            e1
        }
        _ => {
            if let Some(text) = symbol(name) {
                out.push(Sub {
                    range: i..e,
                    text: text.into(),
                });
            } else {
                // Unknown: the name stays literal, and so do `{…}` groups it
                // takes — their braces are content, not markup to erase.
                e = literal_groups(src, e);
            }
            e
        }
    }
}

/// `i` past any `{…}` groups (with optional spaces between) following a
/// literal command.
fn literal_groups(src: &str, mut e: usize) -> usize {
    let b = src.as_bytes();
    loop {
        let mut j = e;
        while j < b.len() && matches!(b[j], b' ' | b'\t') {
            j += 1;
        }
        if b.get(j) != Some(&b'{') {
            return e;
        }
        let Ok((_, end)) = group(src, j) else {
            return e;
        };
        e = end;
    }
}

/// A `{…}` group at `i`: (inner range, index past `}`).
fn group(src: &str, i: usize) -> Result<(Range<usize>, usize), ()> {
    let b = src.as_bytes();
    let mut depth = 1;
    let mut j = i + 1;
    while j < b.len() {
        match b[j] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            b'\\' => j += 1,
            _ => {}
        }
        if depth == 0 {
            return Ok((i + 1..j, j + 1));
        }
        j += 1;
    }
    Err(())
}

/// One command argument at `i` (a `{…}` group, a `\command`, or a single
/// char): (content range, index past it). Leading spaces are skipped.
fn arg(src: &str, mut i: usize) -> Result<(Range<usize>, usize), ()> {
    let b = src.as_bytes();
    while i < b.len() && matches!(b[i], b' ' | b'\t') {
        i += 1;
    }
    match b.get(i) {
        None => Err(()),
        Some(b'{') => group(src, i),
        Some(b'\\') => {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_alphabetic() {
                j += 1;
            }
            if j == i + 1 {
                j = (i + 2).min(b.len());
            }
            Ok((i..j, j))
        }
        Some(_) => {
            let len = src[i..].chars().next().map_or(1, char::len_utf8);
            Ok((i..i + len, i + len))
        }
    }
}

/// `^atom` / `_atom` at `i`; returns where scanning resumes. The whole atom
/// substitutes or the `^`/`_` stays literal — no half-rendered scripts.
fn script(src: &str, i: usize, out: &mut Vec<Sub>) -> usize {
    let sup = src.as_bytes()[i] == b'^';
    let Ok((a, e)) = arg(src, i + 1) else {
        return i + 1;
    };
    // A `\cmd` atom maps via its symbol first (`x^\alpha` → xᵅ).
    let body = if src[a.clone()].starts_with('\\') {
        symbol(&src[a.clone()][1..]).map(str::to_string)
    } else {
        None
    };
    let body = body.as_deref().unwrap_or(&src[a]);
    match super_all(body, sup) {
        Some(text) => {
            out.push(Sub { range: i..e, text });
            e
        }
        None => i + 1,
    }
}

/// `body` in super- or subscript, or `None` when a char has no glyph.
fn super_all(body: &str, sup: bool) -> Option<String> {
    body.chars()
        .map(|c| if sup { sup_char(c) } else { sub_char(c) })
        .collect()
}

fn sup_char(c: char) -> Option<char> {
    Some(match c {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        '+' => '⁺',
        '-' | '−' => '⁻',
        '=' => '⁼',
        '(' => '⁽',
        ')' => '⁾',
        'a' => 'ᵃ',
        'b' => 'ᵇ',
        'c' => 'ᶜ',
        'd' => 'ᵈ',
        'e' => 'ᵉ',
        'f' => 'ᶠ',
        'g' => 'ᵍ',
        'h' => 'ʰ',
        'i' => 'ⁱ',
        'j' => 'ʲ',
        'k' => 'ᵏ',
        'l' => 'ˡ',
        'm' => 'ᵐ',
        'n' => 'ⁿ',
        'o' => 'ᵒ',
        'p' => 'ᵖ',
        'r' => 'ʳ',
        's' => 'ˢ',
        't' => 'ᵗ',
        'u' => 'ᵘ',
        'v' => 'ᵛ',
        'w' => 'ʷ',
        'x' => 'ˣ',
        'y' => 'ʸ',
        'z' => 'ᶻ',
        'A' => 'ᴬ',
        'B' => 'ᴮ',
        'D' => 'ᴰ',
        'E' => 'ᴱ',
        'G' => 'ᴳ',
        'H' => 'ᴴ',
        'I' => 'ᴵ',
        'J' => 'ᴶ',
        'K' => 'ᴷ',
        'L' => 'ᴸ',
        'M' => 'ᴹ',
        'N' => 'ᴺ',
        'O' => 'ᴼ',
        'P' => 'ᴾ',
        'R' => 'ᴿ',
        'T' => 'ᵀ',
        'U' => 'ᵁ',
        'V' => 'ⱽ',
        'W' => 'ᵂ',
        'α' => 'ᵅ',
        'β' => 'ᵝ',
        'γ' => 'ᵞ',
        'δ' => 'ᵟ',
        'ε' => 'ᵋ',
        'θ' => 'ᶿ',
        'ι' => 'ᶥ',
        'φ' => 'ᵠ',
        'χ' => 'ᵡ',
        _ => return None,
    })
}

fn sub_char(c: char) -> Option<char> {
    Some(match c {
        '0' => '₀',
        '1' => '₁',
        '2' => '₂',
        '3' => '₃',
        '4' => '₄',
        '5' => '₅',
        '6' => '₆',
        '7' => '₇',
        '8' => '₈',
        '9' => '₉',
        '+' => '₊',
        '-' | '−' => '₋',
        '=' => '₌',
        '(' => '₍',
        ')' => '₎',
        'a' => 'ₐ',
        'e' => 'ₑ',
        'h' => 'ₕ',
        'i' => 'ᵢ',
        'j' => 'ⱼ',
        'k' => 'ₖ',
        'l' => 'ₗ',
        'm' => 'ₘ',
        'n' => 'ₙ',
        'o' => 'ₒ',
        'p' => 'ₚ',
        'r' => 'ᵣ',
        's' => 'ₛ',
        't' => 'ₜ',
        'u' => 'ᵤ',
        'v' => 'ᵥ',
        'x' => 'ₓ',
        'β' => 'ᵦ',
        'γ' => 'ᵧ',
        'ρ' => 'ᵨ',
        'φ' => 'ᵩ',
        'χ' => 'ᵪ',
        _ => return None,
    })
}

/// `body` in double-struck capitals/digits, or `None` for anything else.
fn double_struck(body: &str) -> Option<String> {
    body.chars()
        .map(|c| {
            Some(match c {
                'C' => 'ℂ',
                'H' => 'ℍ',
                'N' => 'ℕ',
                'P' => 'ℙ',
                'Q' => 'ℚ',
                'R' => 'ℝ',
                'Z' => 'ℤ',
                'A'..='B' | 'D'..='G' | 'I'..='M' | 'O' | 'S'..='Y' => {
                    char::from_u32(0x1d538 + c as u32 - 'A' as u32)?
                }
                'a'..='z' => char::from_u32(0x1d552 + c as u32 - 'a' as u32)?,
                '0'..='9' => char::from_u32(0x1d7d8 + c as u32 - '0' as u32)?,
                _ => return None,
            })
        })
        .collect()
}

/// Argument-free command → its Unicode text.
fn symbol(name: &str) -> Option<&'static str> {
    Some(match name {
        // Escaped punctuation.
        "{" | "lbrace" => "{",
        "}" | "rbrace" => "}",
        "$" | "dollar" => "$",
        "%" => "%",
        "&" => "&",
        "#" => "#",
        "_" => "_",
        "|" | "Vert" => "‖",
        "," => "\u{2009}",
        ";" | ":" | " " => " ",
        "!" => "",
        "quad" => " ",
        "qquad" => "  ",
        // Greek.
        "alpha" => "α",
        "beta" => "β",
        "gamma" => "γ",
        "delta" => "δ",
        "epsilon" => "ϵ",
        "varepsilon" => "ε",
        "zeta" => "ζ",
        "eta" => "η",
        "theta" => "θ",
        "vartheta" => "ϑ",
        "iota" => "ι",
        "kappa" => "κ",
        "varkappa" => "ϰ",
        "lambda" => "λ",
        "mu" => "μ",
        "nu" => "ν",
        "xi" => "ξ",
        "pi" => "π",
        "varpi" => "ϖ",
        "rho" => "ρ",
        "varrho" => "ϱ",
        "sigma" => "σ",
        "varsigma" => "ς",
        "tau" => "τ",
        "upsilon" => "υ",
        "phi" => "ϕ",
        "varphi" => "φ",
        "chi" => "χ",
        "psi" => "ψ",
        "omega" => "ω",
        "Gamma" => "Γ",
        "Delta" => "Δ",
        "Theta" => "Θ",
        "Lambda" => "Λ",
        "Xi" => "Ξ",
        "Pi" => "Π",
        "Sigma" => "Σ",
        "Upsilon" => "Υ",
        "Phi" => "Φ",
        "Psi" => "Ψ",
        "Omega" => "Ω",
        // Operators and relations.
        "sum" => "∑",
        "prod" => "∏",
        "coprod" => "∐",
        "int" => "∫",
        "iint" => "∬",
        "iiint" => "∭",
        "oint" => "∮",
        "bigcup" => "⋃",
        "bigcap" => "⋂",
        "bigsqcup" => "⨆",
        "bigoplus" => "⨁",
        "bigotimes" => "⨂",
        "bigodot" => "⨀",
        "bigvee" => "⋁",
        "bigwedge" => "⋀",
        "nabla" => "∇",
        "partial" => "∂",
        "infty" => "∞",
        "pm" => "±",
        "mp" => "∓",
        "times" => "×",
        "div" => "÷",
        "cdot" | "cdotp" | "centerdot" => "·",
        "ast" => "∗",
        "star" => "⋆",
        "circ" => "∘",
        "bullet" => "∙",
        "oplus" => "⊕",
        "ominus" => "⊖",
        "otimes" => "⊗",
        "oslash" => "⊘",
        "odot" => "⊙",
        "dagger" | "dag" => "†",
        "ddagger" | "ddag" => "‡",
        "cap" => "∩",
        "cup" => "∪",
        "uplus" => "⊎",
        "sqcap" => "⊓",
        "sqcup" => "⊔",
        "vee" | "lor" => "∨",
        "wedge" | "land" => "∧",
        "setminus" | "smallsetminus" => "∖",
        "wr" => "≀",
        "diamond" => "⋄",
        "bigtriangleup" => "△",
        "bigtriangledown" => "▽",
        "triangleleft" => "◁",
        "triangleright" => "▷",
        "lhd" => "⊲",
        "rhd" => "⊳",
        "unlhd" => "⊴",
        "unrhd" => "⊵",
        "amalg" => "⨿",
        "lnot" | "neg" => "¬",
        "le" | "leq" | "leqslant" => "≤",
        "ge" | "geq" | "geqslant" => "≥",
        "ne" | "neq" => "≠",
        "equiv" => "≡",
        "approx" => "≈",
        "cong" => "≅",
        "simeq" => "≃",
        "sim" => "∼",
        "propto" => "∝",
        "asymp" => "≍",
        "doteq" => "≐",
        "models" => "⊨",
        "prec" => "≺",
        "succ" => "≻",
        "preceq" => "⪯",
        "succeq" => "⪰",
        "ll" => "≪",
        "gg" => "≫",
        "subset" => "⊂",
        "supset" => "⊃",
        "subseteq" | "subseteqq" => "⊆",
        "supseteq" | "supseteqq" => "⊇",
        "nsubseteq" => "⊈",
        "nsupseteq" => "⊉",
        "sqsubseteq" => "⊑",
        "sqsupseteq" => "⊒",
        "in" => "∈",
        "ni" | "owns" => "∋",
        "notin" => "∉",
        "vdash" => "⊢",
        "dashv" => "⊣",
        "perp" | "bot" => "⊥",
        "mid" | "shortmid" => "∣",
        "parallel" | "shortparallel" => "∥",
        "nmid" => "∤",
        "nparallel" => "∦",
        "bowtie" | "Join" => "⋈",
        "smile" => "⌣",
        "frown" => "⌢",
        "top" => "⊤",
        "vdots" => "⋮",
        "cdots" => "⋯",
        "ldots" | "dots" | "dotsc" | "dotsb" | "ldotp" => "…",
        "ddots" => "⋱",
        "therefore" => "∴",
        "because" => "∵",
        "prime" => "′",
        // Arrows.
        "leftarrow" | "gets" => "←",
        "rightarrow" | "to" => "→",
        "Leftarrow" => "⇐",
        "Rightarrow" | "implies" => "⇒",
        "Leftrightarrow" | "iff" => "⇔",
        "leftrightarrow" => "↔",
        "mapsto" => "↦",
        "hookleftarrow" => "↩",
        "hookrightarrow" => "↪",
        "leftharpoonup" => "↼",
        "leftharpoondown" => "↽",
        "rightharpoonup" => "⇀",
        "rightharpoondown" => "⇁",
        "longleftarrow" => "⟵",
        "longrightarrow" => "⟶",
        "Longleftarrow" => "⟸",
        "Longrightarrow" => "⟹",
        "Longleftrightarrow" | "impliedby" => "⟺",
        "longmapsto" => "⟼",
        "uparrow" => "↑",
        "downarrow" => "↓",
        "Uparrow" => "⇑",
        "Downarrow" => "⇓",
        "updownarrow" => "↕",
        "Updownarrow" => "⇕",
        "nearrow" => "↗",
        "searrow" => "↘",
        "swarrow" => "↙",
        "nwarrow" => "↖",
        "leadsto" => "⇝",
        // Named functions keep their name upright — the italic math style
        // already sets them apart.
        "sin" => "sin",
        "cos" => "cos",
        "tan" => "tan",
        "cot" => "cot",
        "sec" => "sec",
        "csc" => "csc",
        "arcsin" => "arcsin",
        "arccos" => "arccos",
        "arctan" => "arctan",
        "sinh" => "sinh",
        "cosh" => "cosh",
        "tanh" => "tanh",
        "coth" => "coth",
        "log" => "log",
        "ln" => "ln",
        "lg" => "lg",
        "lim" => "lim",
        "limsup" => "lim sup",
        "liminf" => "lim inf",
        "sup" => "sup",
        "inf" => "inf",
        "max" => "max",
        "min" => "min",
        "arg" => "arg",
        "deg" => "deg",
        "det" => "det",
        "dim" => "dim",
        "exp" => "exp",
        "gcd" => "gcd",
        "hom" => "hom",
        "ker" => "ker",
        "Pr" => "Pr",
        "mod" => "mod",
        "bmod" => "mod",
        // Delimiters and misc.
        "langle" => "⟨",
        "rangle" => "⟩",
        "lceil" => "⌈",
        "rceil" => "⌉",
        "lfloor" => "⌊",
        "rfloor" => "⌋",
        "vert" => "|",
        "backslash" => "\\",
        "forall" => "∀",
        "exists" => "∃",
        "nexists" => "∄",
        "emptyset" | "varnothing" => "∅",
        "aleph" => "ℵ",
        "hbar" | "hslash" => "ℏ",
        "ell" => "ℓ",
        "wp" => "℘",
        "Re" => "ℜ",
        "Im" => "ℑ",
        "angle" => "∠",
        "measuredangle" => "∡",
        "triangle" => "△",
        "square" | "Box" => "□",
        "blacksquare" => "■",
        "Diamond" => "◇",
        "degree" => "°",
        "S" => "§",
        "P" => "¶",
        "copyright" => "©",
        "checkmark" => "✓",
        "colon" => ":",
        "displaystyle" | "textstyle" | "scriptstyle" | "scriptscriptstyle" | "limits"
        | "nolimits" | "nonumber" => "",
        _ => return None,
    })
}
