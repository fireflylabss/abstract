//! Spellcheck: Hunspell dictionaries (en_US + pt_BR) via `spellbook`, loaded
//! lazily off the UI thread; per-line misspelled ranges for the wavy
//! underline; a personal dictionary persisted next to the settings file.
//!
//! Dictionaries ship zlib-compressed in the binary (see `build.rs`): ~6 MB
//! raw → ~1.6 MB embedded, inflated once on first use.

use std::collections::{BTreeSet, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use spellbook::Dictionary;

use crate::md::{Analysis, Kind};

/// Quiet period after the last edit before a dirty line is re-checked.
pub const DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(120);

const MAX_USER_WORDS: usize = 10_000;
/// Bytes flags that mark a range as not-prose: syntax, code, frontmatter,
/// math, concealed link destinations.
const SKIP_FLAGS: u16 = crate::md::CODE | crate::md::MARK | crate::md::MUTED;

/// Bitmask of the dictionaries a word is checked against.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Langs(pub u8);

impl Langs {
    pub const EN: Self = Self(1);
    pub const PT: Self = Self(2);

    fn has(self, l: Self) -> bool {
        self.0 & l.0 != 0
    }
}

/// `spell-lang` setting: `auto` (default) follows the UI language.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpellLang {
    Auto,
    En,
    PtBr,
    Both,
}

impl SpellLang {
    pub fn parse(s: &str) -> Self {
        match s {
            "en" => Self::En,
            "pt-BR" | "pt" => Self::PtBr,
            "both" => Self::Both,
            _ => Self::Auto,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::En => "en",
            Self::PtBr => "pt-BR",
            Self::Both => "both",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::En,
            Self::En => Self::PtBr,
            Self::PtBr => Self::Both,
            Self::Both => Self::Auto,
        }
    }

    /// Resolve `Auto` against the active UI language.
    pub fn langs(self) -> Langs {
        match self {
            Self::Auto => match crate::i18n::current() {
                crate::i18n::Lang::En => Langs::EN,
                crate::i18n::Lang::PtBr => Langs::PT,
            },
            Self::En => Langs::EN,
            Self::PtBr => Langs::PT,
            Self::Both => Langs::EN | Langs::PT,
        }
    }
}

impl std::ops::BitOr for Langs {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Both dictionaries plus the user's personal word list. A dictionary that
/// fails to parse is absent rather than flagging noise.
pub struct Engine {
    en: Option<Dictionary>,
    pt: Option<Dictionary>,
    /// Lowercase words the user added; consulted before flagging an error.
    user: RwLock<BTreeSet<String>>,
    /// Bumped on `add_user_word` so content-keyed caches rebuild.
    epoch: AtomicU32,
}

fn inflate(z: &[u8]) -> Option<String> {
    use std::io::Read;
    let mut out = String::new();
    flate2::read::ZlibDecoder::new(z)
        .read_to_string(&mut out)
        .ok()
        .map(|_| out)
}

fn load_dict(aff_z: &[u8], dic_z: &[u8]) -> Option<Dictionary> {
    let aff = inflate(aff_z)?;
    let dic = inflate(dic_z)?;
    Dictionary::new(&aff, &dic).ok()
}

/// One `$XDG_CONFIG_HOME/abstract/userdict` file, one word per line.
fn user_file() -> PathBuf {
    crate::store::xdg("XDG_CONFIG_HOME", ".config")
        .join("abstract")
        .join("userdict")
}

impl Engine {
    /// Blocking: inflates + parses both dictionaries and reads the personal
    /// dictionary. Call from a background task.
    pub fn load() -> Self {
        let en = load_dict(
            include_bytes!(concat!(env!("OUT_DIR"), "/dict/en_US.aff.z")),
            include_bytes!(concat!(env!("OUT_DIR"), "/dict/en_US.dic.z")),
        );
        let pt = load_dict(
            include_bytes!(concat!(env!("OUT_DIR"), "/dict/pt_BR.aff.z")),
            include_bytes!(concat!(env!("OUT_DIR"), "/dict/pt_BR.dic.z")),
        );
        let user = std::fs::read_to_string(user_file())
            .map(|s| {
                s.lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(str::to_lowercase)
                    .collect::<BTreeSet<String>>()
            })
            .unwrap_or_default();
        Self {
            en,
            pt,
            user: RwLock::new(user),
            epoch: AtomicU32::new(0),
        }
    }

    pub fn epoch(&self) -> u32 {
        self.epoch.load(Ordering::Relaxed)
    }

    /// `true` when the word is known in any selected dictionary — or was
    /// added to the personal dictionary.
    pub fn check(&self, word: &str, langs: Langs) -> bool {
        if let Ok(user) = self.user.read()
            && user.contains(word.to_lowercase().as_str())
        {
            return true;
        }
        if langs.has(Langs::EN)
            && let Some(d) = &self.en
            && d.check(word)
        {
            return true;
        }
        if langs.has(Langs::PT)
            && let Some(d) = &self.pt
            && d.check(word)
        {
            return true;
        }
        false
    }

    /// Up to `n` suggestions for a misspelled word, best guesses first.
    pub fn suggest(&self, word: &str, langs: Langs, n: usize) -> Vec<String> {
        // `Dictionary::suggest` clears its output vec, so collect per
        // dictionary, then interleave so each language keeps its best
        // guesses visible when both are enabled.
        let mut en = Vec::new();
        let mut pt = Vec::new();
        if langs.has(Langs::EN)
            && let Some(d) = &self.en
        {
            d.suggest(word, &mut en);
        }
        if langs.has(Langs::PT)
            && let Some(d) = &self.pt
        {
            d.suggest(word, &mut pt);
        }
        let mut out = Vec::new();
        let mut en = en.into_iter();
        let mut pt = pt.into_iter();
        loop {
            match (en.next(), pt.next()) {
                (None, None) => break,
                (a, b) => {
                    out.extend(a);
                    out.extend(b);
                }
            }
        }
        // Keep each dictionary's quality order; dedup preserving position.
        let mut seen = HashSet::new();
        out.retain(|s| seen.insert(s.clone()));
        out.truncate(n);
        out
    }

    /// Add a word to the personal dictionary (persisted by `save_user`).
    pub fn add_user_word(&self, word: &str) {
        if let Ok(mut user) = self.user.write() {
            let word = word.to_lowercase();
            if user.len() < MAX_USER_WORDS && !word.is_empty() {
                user.insert(word);
                self.epoch.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Persist the personal dictionary; blocking, call off the UI thread.
    pub fn save_user(&self) {
        let Ok(user) = self.user.read() else {
            return;
        };
        let body: String = user.iter().map(|w| format!("{w}\n")).collect();
        if let Err(err) = crate::store::write_atomic(&user_file(), body.as_bytes()) {
            eprintln!("abstract: failed to save userdict: {err}");
        }
    }
}

static SHARED: OnceLock<Arc<Engine>> = OnceLock::new();

/// Process-wide engine; the first caller (a background task) pays the load.
pub fn shared() -> Arc<Engine> {
    SHARED.get_or_init(|| Arc::new(Engine::load())).clone()
}

/// Cache key for a line's misspell ranges: the line text plus the
/// configuration that changes results, so edits/config changes invalidate
/// without bookkeeping.
pub fn line_key(line: &str, langs: Langs, epoch: u32) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (langs.0, epoch, line).hash(&mut h);
    h.finish()
}

fn is_word_char(c: char) -> bool {
    c.is_alphabetic() || matches!(c, '\'' | '\u{2019}')
}

/// Word characters that can also appear inside a URL/file path token.
fn is_urlish_char(c: char) -> bool {
    c.is_alphanumeric()
        || matches!(
            c,
            '.' | '/' | ':' | '@' | '~' | '#' | '?' | '&' | '=' | '%' | '-' | '_' | '+'
        )
}

/// `true` when the word token containing `[s, e)` looks like a URL or a path
/// (contains `.`, `:`, `/` or `@` once the URL-ish neighbours are included).
fn urlish(line: &str, mut s: usize, mut e: usize) -> bool {
    while s > 0 && line[..s].chars().next_back().is_some_and(is_urlish_char) {
        s -= line[..s].chars().next_back().unwrap().len_utf8();
    }
    while e < line.len() && line[e..].chars().next().is_some_and(is_urlish_char) {
        e += line[e..].chars().next().unwrap().len_utf8();
    }
    let tok = line[s..e].trim_end_matches('.');
    tok.contains(['.', ':', '/', '@'])
}

/// `true` when a word is prose worth checking. Filters words with digits,
/// ALL-CAPS acronyms, camelCase identifiers and URL/path-looking tokens.
fn checkable(line: &str, s: usize, e: usize) -> Option<(usize, usize)> {
    // A word glued to digits (`abc123`) is an identifier, not prose.
    let prev = line[..s].chars().next_back();
    let next = line[e..].chars().next();
    if prev.is_some_and(|c| c.is_alphanumeric()) || next.is_some_and(|c| c.is_alphanumeric()) {
        return None;
    }
    let quote = |c: char| matches!(c, '\'' | '\u{2019}');
    let raw = &line[s..e];
    let word = raw.trim_matches(quote);
    if word.is_empty() {
        return None;
    }
    let s = s + raw.len() - raw.trim_start_matches(quote).len();
    let e = e - (raw.len() - raw.trim_end_matches(quote).len());
    let word = &line[s..e];
    if word.chars().count() < 2 {
        return None;
    }
    if word.chars().any(|c| c.is_numeric()) {
        return None;
    }
    if word.chars().all(|c| c.is_uppercase()) {
        return None;
    }
    let mut it = word.chars().peekable();
    while let Some(c) = it.next() {
        if c.is_lowercase() && it.peek().is_some_and(|n| n.is_uppercase()) {
            return None;
        }
    }
    if urlish(line, s, e) {
        return None;
    }
    Some((s, e))
}

/// Misspelled word ranges in line `ix`, relative to the line start. Words in
/// code, frontmatter, math, link destinations and `[[wiki]]`/`![]()` targets
/// are skipped via the analysis flags and recorded ranges.
pub fn scan_line(
    a: &Analysis,
    text: &str,
    ix: usize,
    engine: &Engine,
    langs: Langs,
) -> Vec<Range<usize>> {
    let (range, kind) = &a.lines[ix];
    if *kind == Kind::Code {
        return Vec::new();
    }
    let (start, end) = (range.start, range.end);
    let line = &text[start..end];
    let mut bad = Vec::new();
    let mut i = 0;
    while i < line.len() {
        let c = line[i..].chars().next().unwrap();
        if !is_word_char(c) {
            i += c.len_utf8();
            continue;
        }
        let s = i;
        i += c.len_utf8();
        while i < line.len() && is_word_char(line[i..].chars().next().unwrap()) {
            i += line[i..].chars().next().unwrap().len_utf8();
        }
        let e = i;
        let Some((ws, we)) = checkable(line, s, e) else {
            continue;
        };
        let (abs_s, abs_e) = (start + ws, start + we);
        let skip = a.flags[abs_s..abs_e].iter().any(|f| f & SKIP_FLAGS != 0)
            || a.wiki_links
                .iter()
                .any(|l| l.target.start < abs_e && abs_s < l.target.end)
            || a.images
                .iter()
                .any(|im| im.src.start < abs_e && abs_s < im.src.end);
        if !skip && !engine.check(&line[ws..we], langs) {
            bad.push(ws..we);
        }
    }
    bad
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::md::Analyzer;

    fn engine() -> Engine {
        Engine {
            en: None,
            pt: None,
            user: RwLock::new(BTreeSet::new()),
            epoch: AtomicU32::new(0),
        }
    }

    /// Everything is "misspelled" without dictionaries — drives the zone
    /// tests; with dicts, only real typos would surface.
    fn bad_words(text: &str, ix: usize) -> Vec<String> {
        let mut an = Analyzer::new();
        let a = an.analyze(text);
        let eng = engine();
        scan_line(&a, text, ix, &eng, Langs::EN | Langs::PT)
            .iter()
            .map(|r| text[a.lines[ix].0.start + r.start..a.lines[ix].0.start + r.end].to_string())
            .collect()
    }

    #[test]
    fn plain_words_flagged() {
        let t = "hello world";
        assert_eq!(bad_words(t, 0), vec!["hello", "world"]);
    }

    #[test]
    fn code_spans_and_fences_skipped() {
        let t = "helo `wrng wordd` end\n\n```\nwrng codee\n```";
        assert_eq!(bad_words(t, 0), vec!["helo", "end"]);
        assert!(bad_words(t, 2).is_empty());
    }

    #[test]
    fn urls_and_link_targets_skipped() {
        let t = "see https://exampel.com/x and [labell](https://tg-url.io) endd";
        assert_eq!(bad_words(t, 0), vec!["see", "and", "labell", "endd"]);
    }

    #[test]
    fn wiki_link_target_skipped() {
        let t = "see [[noott targget]] andd [[ok|aliiass]] finee";
        assert_eq!(bad_words(t, 0), vec!["see", "andd", "aliiass", "finee"]);
    }

    #[test]
    fn frontmatter_skipped() {
        let t = "---\ntitlee: badd\n---\nreal helo";
        assert!(bad_words(t, 0).is_empty());
        assert_eq!(bad_words(t, 3), vec!["real", "helo"]);
    }

    #[test]
    fn digits_caps_camel_skipped() {
        let t = "abc123 HTTP camelCase iPhone helo";
        assert_eq!(bad_words(t, 0), vec!["helo"]);
    }

    #[test]
    fn image_src_skipped() {
        // Image syntax (src and alt) is flagged MUTED/MARK, so it is never
        // checked — only plain words around it are.
        let t = "text ![[foto-bad.png]] andd ![altt](imagge.png) moree";
        assert_eq!(bad_words(t, 0), vec!["text", "andd", "moree"]);
    }

    #[test]
    fn user_word_not_flagged() {
        let mut an = Analyzer::new();
        let t = "helo wrld";
        let a = an.analyze(t);
        let eng = engine();
        eng.add_user_word("helo");
        let bad = scan_line(&a, t, 0, &eng, Langs::EN | Langs::PT);
        assert_eq!(bad, vec![5..9]);
    }

    #[test]
    fn lang_mask_limits_dicts() {
        // Without dicts everything flags; this asserts mask plumbing reaches
        // Engine::check (a word in `user` always passes regardless of mask).
        let eng = engine();
        eng.add_user_word("Quokka");
        assert!(eng.check("quokka", Langs::EN));
        assert!(eng.check("QUOKKA", Langs::PT));
    }
}

#[cfg(test)]
mod scan_probe {
    use super::*;

    #[test]
    #[ignore]
    fn scan_probe() {
        let mut an = crate::md::Analyzer::new();
        let t = "camelCase HTTP abc123 sao ignorados.";
        let a = an.analyze(t);
        let eng = Engine::load();
        let bad = scan_line(&a, t, 0, &eng, Langs::EN | Langs::PT);
        for r in &bad {
            println!("flagged: {:?} = {:?}", r, &t[r.clone()]);
        }
        assert!(
            bad.iter().all(|r| &t[r.clone()] != "ignorados"),
            "ignorados flagged!"
        );
    }
}
