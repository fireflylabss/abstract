# Changelog

All notable changes to abstract are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.0] - 2026-10-02

### Added
- Note tabs: every open note gets a tab (with its own cursor, scroll and
  undo state) on a new strip under the toolbar, and `Alt+←/→` (`Cmd+[`/`Cmd+]`)
  walks the history of opened notes like a browser. `Ctrl+Tab` or
  `Cmd+Shift+]`/`[` cycles tabs; `Cmd/Ctrl+W` or a middle-click closes one.
  Tabs, the active tab and their positions are restored across launches.
- Outline panel on the right listing the open note's headings, indented by
  level — click a heading to jump to it. Toggle it from the toolbar or with
  `Cmd/Ctrl+Shift+O`; the state persists.
- Command palette: a `Cmd/Ctrl+P` query starting with `>` fuzzy-finds app
  commands (localized labels, with their real shortcuts) and runs them.
- YAML front matter: a `---`-fenced block at the top of a note collapses to
  a dimmed `Properties (n keys)` row and no longer renders as a heading or
  divider — move the caret into it to edit the raw YAML.
- Link preview: holding the link modifier (`Cmd` on macOS, `Ctrl` elsewhere)
  over a `[[link]]` pops a card with the target note's title and first
  lines, or a click-to-create hint for notes that don't exist yet.
- Tags: `#tag` anywhere in body text (never in code) collects into a new
  collapsible TAGS sidebar section — click one to filter the note list.
  Typing `#` autocompletes tag names.
- Pinned notes: right-click a note to pin it — pinned notes appear in a
  PINNED group on top of the note list and persist per space.
- Daily note: `Cmd/Ctrl+Shift+D` opens (creating, when needed) today's
  `YYYY-MM-DD.md` under the daily-notes folder, seeded with a localized
  date heading; the folder name is configurable in Settings.
- Export as HTML: a note exports to a standalone styled page via the save
  dialog — callouts, tables, task lists, footnotes and wiki-links resolved
  to relative links, with private `%%comments%%` left out — and **Copy as
  HTML** puts formatted rich text on the clipboard.
- Distribution manifests: a Homebrew cask (macOS arm + Intel), a Flatpak
  manifest with AppStream metainfo, and a winget manifest set for the
  portable Windows `.exe`, all under `packaging/` (see
  `docs/RELEASING.md`).

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

[Unreleased]: https://github.com/fireflylabss/abstract/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/fireflylabss/abstract/compare/v0.1.3...v0.2.0
[0.1.3]: https://github.com/fireflylabss/abstract/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/fireflylabss/abstract/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/fireflylabss/abstract/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/fireflylabss/abstract/releases/tag/v0.1.0
