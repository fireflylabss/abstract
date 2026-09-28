# Changelog

All notable changes to abstract are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed
- Renaming a note also rewrites links to itself inside that note, and links
  in a note you switch away from while the rename is running.
- If some links can't be rewritten after a rename, a notice says so instead
  of failing silently.

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

[Unreleased]: https://github.com/fireflylabss/abstract/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/fireflylabss/abstract/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/fireflylabss/abstract/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/fireflylabss/abstract/releases/tag/v0.1.0
