# Changelog

All notable changes to abstract are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Note tabs: several notes open at once under a compact tab strip above the
  editor — each tab shows the note title, an unsaved dot and a close button
  on hover, and scrolls sideways when it overflows. Clicking a note reuses
  the active tab; `Cmd/Ctrl`+click, middle-click and "Open in new tab" in
  the sidebar context menu open a new tab, as does `Cmd`+clicking a
  [[wiki-link]].
- Tab shortcuts: `Cmd/Ctrl`+`W` closes the tab, `Ctrl`+`Tab` /
  `Ctrl`+`Shift`+`Tab` cycle, `Cmd/Ctrl`+`1`..`9` jumps to a tab (`9` = last),
  `Cmd/Ctrl`+`Shift`+`T` reopens the last closed tab.
- Open tabs, the active one and each tab's cursor/scroll are restored when
  the app or a space is reopened; every tab keeps its own autosave and
  word count.

### Added
- LaTeX math: inline `$…$` and block `$$…$$` spans are recognized in notes.
  Delimiters hide until the caret touches the span (like `**bold**`), money
  like `R$ 10` and `\$` escapes stay literal, and nothing inside code spans,
  fences or tables is parsed as math.
- In the editor, math renders as a Unicode approximation — Greek letters,
  `\sum`/`∫`-style operators, relations, arrows, `\sqrt{}`→`√(…)`,
  `\frac{a}{b}`→`(a)/(b)`, and `^`/`_` scripts become Unicode
  super/subscripts — in an italic serif look; unknown commands stay
  literal. `$$` blocks render centered on their own line.
- HTML export converts math to real MathML via `pulldown-latex` — browsers
  render it natively, so the file stays self-contained with no CDN or JS.
- Insert submenu gains "Math inline" (`$…$`) and "Math block" (`$$…$$`)
  items that drop the delimiters with the caret inside.

### Added
- Export a note as PDF via "Export as PDF…" next to the HTML export —
  pure-Rust rendering with the bundled Noto fonts embedded: A4 pages,
  headings, styled text and highlights, lists and task checkboxes, quotes,
  callouts, code blocks, tables, images, footnotes and page numbers.
  Generation runs on a background thread with an in-progress notice.

### Added
- Spellcheck for English and Portuguese (Brazil): misspelled words get a
  discreet red wavy underline in the editor. Right-click on a marked word
  for up to five suggestions (applied in one undo step) and "Add to
  dictionary" for a personal dictionary that persists across launches.
  Code, URLs, link and `[[wiki-link]]` targets, frontmatter, math and image
  filenames are never marked, and words with digits, ALL-CAPS acronyms or
  camelCase pass through. Settings → General has the on/off toggle (on by
  default) and the language row — Automático (follows the UI language),
  English, Português (Brasil) or Ambos — applied instantly. Checking runs
  only over the visible area with a per-line cache so typing stays fast on
  large notes.

### Added
- Text width setting in Settings → Editor: narrow (580), medium (700, the
  previous fixed width) or wide (880) column, applied instantly.

### Added
- Line-editing shortcuts: `Alt+↑`/`Alt+↓` move the current or selected
  lines, `Ctrl`/`Cmd`+`Shift+D` duplicates them below and `Ctrl`/`Cmd`+`D`
  selects the word under the caret or jumps to the next occurrence.

### Added
- Typographic substitution while typing (smart quotes): `"`/`'` become
  curly by context, `--` folds into an em dash, and Backspace right after
  a substitution restores the straight characters. Code and frontmatter
  stay literal; toggle in Settings → Editor (on by default).

### Added
- Focus mode (`Cmd/Ctrl`+`Shift`+`Enter` or the toolbar button): hides the
  sidebar, dims the status area until hovered, mutes every paragraph but the
  one holding the caret and keeps the caret centered while typing. `Esc`
  leaves it; state is not persisted.

## [0.1.4] - 2026-10-02

### Added
- Discord Rich Presence: while abstract is open, your Discord profile shows
  an "abstract" activity with the current note title and space name. A
  Discord presence toggle lives in Settings → General (on by default).
- Export a note as a self-contained HTML file — styled, opens in the
  browser ready to print or save as PDF — plus Copy as HTML on the context
  menu.
- User-selectable font in Settings → General: pick the interface and note
  typeface from the bundled fonts or installed system families; code and
  tables always stay in Noto Sans Mono. Applies instantly.
- UI zoom: `Cmd/Ctrl`+`=`/`-`/`0` scales text, chrome, sidebar, palette and
  dialogs together from 50% to 200%, persisted across launches and shown in
  the status area.
- Colored highlights: `=={red}…==`, `=={orange}…==`, `=={green}…==`,
  `=={blue}…==` and `=={purple}…==` (yellow stays the bare `==…==`),
  pickable under Format → Highlight color; unknown names render yellow and
  exported HTML carries the color on the `<mark>`.
- Table structure editing from the context menu: while the caret is inside
  a table, a Table submenu offers insert row above/below, insert column
  left/right and delete row/column — rebuilding the Markdown in a single
  undo step and preserving column alignment.
- Packaging manifests for Homebrew (cask), Flathub (Flatpak metainfo and
  manifest) and winget.

### Changed
- Context menus are smarter: items only appear when usable (Paste is
  disabled with an empty clipboard, Paste plain only with text), a
  right-click selects the sidebar row, only one menu is open at a time,
  menus open with a brief animation and clamp to the window edge.
- UI consistency pass: unified radii, paddings and hover/pressed states
  across panels, menus and dialogs.
- Subtle motion throughout: one easing and a short duration scale
  (100–160 ms) for entrances, exits and hovers on menus, palette, sidebar,
  dialogs and notices — keeping the app's clear look.
- The dependency tree no longer resolves through a yanked crate
  (yoke-derive), unblocking builds and `cargo deny`.

## [0.1.3] - 2026-09-28

### Added
- Self-update. Once a day (can be turned off in Settings) the app checks the
  latest GitHub release and offers to install it. Downloads are verified
  against a minisign-signed `SHA256SUMS`; AppImage, Windows and tarball
  installs update in place, while `.deb`, `.rpm` and AUR installs only get a
  notice.
- Local crash reports. A panic writes a report (version, OS, backtrace), plus
  the text of any save that hadn't finished, to a local `crashes` folder; the
  next launch offers to copy the report or open a prefilled GitHub issue.
  Nothing is uploaded.
- Find and replace in the open note (`Cmd/Ctrl+F`; replace with `Ctrl+H`,
  `Cmd+Alt+F` on macOS). Matches are highlighted in the editor, case folding
  is on by default, and Replace all is a single undo step.
- GFM tables render as a bordered grid with a shaded header row, striped
  rows and the delimiter row's column alignment. Putting the caret in a table
  shows its Markdown source, where `Tab` and `Shift+Tab` realign the columns
  and move between cells (`Tab` in the last cell adds a row). Insert > Table
  adds a 2-column table.
- Settings dialog, from the button at the bottom of the sidebar or
  `Cmd/Ctrl+,`: theme mode, light palettes (Abstract, Paper, Sepia,
  Solarized) and dark palettes (Abstract, Midnight, Nord, Solarized), a Raw
  tables option (off by default) that keeps tables as monospace source, the
  language and update-check options, and an About section with the version
  and project links.
- Footnotes. `[^label]` references and `[^label]: text` definitions show as
  `[label]`, Cmd/Ctrl+click jumps between a reference and its definition, and
  Insert > Footnote adds the next numbered one with its definition at the end
  of the note.

### Changed
- Faster on large notes and spaces: inline Markdown parsing is much cheaper
  (a 1 MB note parses in about 370 ms instead of 2.5 s), backlinks and link
  resolution use a cached index, and saving a note no longer rescans the
  whole space.

## [0.1.2] - 2026-09-28

### Added
- Right-click menus. In the editor: cut, copy, paste, paste as plain text,
  select all, add wiki/external link, search the selection, plus Format,
  Paragraph and Insert submenus. In the sidebar: open, new note/folder here,
  rename, duplicate, copy path / relative path / note link, reveal in the file
  manager, open with the default app and move to Trash.
- Paste images from the clipboard and drop files onto the editor. Images are
  saved to an `attachments/` folder next to the note and inserted as
  `![](attachments/…)`; other files become links.
- Markdown images (`![alt](path)` and `![[image.png]]`) render inline below
  their line; the syntax shows again when the selection touches it.
- Insert > Image opens a file picker.
- Drag notes and folders in the sidebar onto a folder (or the "Notes" header)
  to move them; a moved note's relative image paths are updated.
- Note status button in the toolbar: save state, words, characters, reading
  time, selection word count, last modified, reveal and copy path.
- `==highlight==` (Format > Highlight, `Cmd/Ctrl+Shift+H`) and hidden
  `%%comments%%` (Format > Hidden comment, `Cmd/Ctrl+/`), which render dimmed;
  both hide their delimiters until the selection touches them.
- Callouts: `> [!note]`, `[!tip]`, `[!warning]` and `[!danger]` (plus their
  usual aliases) render as a tinted box with a coloured title. Paragraph >
  Callout toggles one.

### Fixed
- Renaming a note also rewrites links to itself inside that note, and links
  in a note you switch away from while the rename is running.
- If some links can't be rewritten after a rename, a notice says so instead
  of failing silently.
- First-run tour: the Skip / Back / Next buttons rendered without their
  labels, and the bubble was centered on top of the control it pointed at.
- Tour shortcuts now show `Cmd` on macOS instead of `Ctrl`.
- The `>` of a block quote's second and later lines is concealed like the
  first one's.
- Renaming a note, by F2, the sidebar or its title, now rewrites the
  `[[links]]` pointing to it across the space instead of leaving them to
  create a new empty note. Links stay untouched when another note shares the
  old or the new name.

## [0.1.1] - 2026-09-26

### Added
- Linux aarch64 builds (`tar.gz`, `.deb`, `.AppImage`, `.rpm`).
- `.AppImage` and `.rpm` packages for Linux x86_64.
- macOS builds are signed with a Developer ID and notarized when the signing
  secrets are configured in the release workflow; unsigned (ad-hoc) builds are
  still produced otherwise.
- `.SRCINFO` committed next to each AUR `PKGBUILD`.
- This changelog.

### Changed
- AUR `abstract-editor` / `abstract-editor-bin` bumped to `pkgrel=2` for the
  re-tagged v0.1.0.

## [0.1.0] - 2026-09-26

First public release.

### Added
- Local-first markdown notes editor rendered on the GPU with GPUI, with a
  sidebar of spaces and notes, a first-run tour and light/dark themes.
- Markdown analysis: headings, emphasis, fenced code blocks with syntax
  highlighting, painted bullets and clickable task checkboxes.
- `[[wiki-links]]` with autocomplete, Ctrl/Cmd+click to open or create the
  target note, and a backlinks panel under the editor.
- Global note search palette (Ctrl/Cmd+P).
- Live reload when the space folder changes on disk.
- i18n (English, pt-BR) with locale detection and an in-app switch.
- Bundled Noto Sans / Noto Sans Mono fonts.
- macOS: native traffic lights, app icon, macOS keybindings and graceful quit.
- Windows: native window control areas, icon embedded in the `.exe`.
- Linux: Wayland and X11 support; `.desktop` entry and icon.
- Release artifacts: macOS `.dmg` / `.app.zip` (Apple Silicon and Intel),
  Linux x86_64 `tar.gz` and `.deb`, Windows x86_64 `.exe` and `.zip`,
  `SHA256SUMS`.
- AUR packages `abstract-editor` (source) and `abstract-editor-bin`.

[Unreleased]: https://github.com/fireflylabss/abstract/compare/v0.1.4...HEAD
[0.1.4]: https://github.com/fireflylabss/abstract/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/fireflylabss/abstract/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/fireflylabss/abstract/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/fireflylabss/abstract/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/fireflylabss/abstract/releases/tag/v0.1.0
