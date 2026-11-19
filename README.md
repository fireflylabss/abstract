# <img src="assets/logo.png" width="96" alt="abstract logo"> abstract

**A minimal, local-first markdown notes editor — GPU-rendered with [Rune](https://github.com/anomalyco/rune).**

[![ci](https://github.com/fireflylabss/abstract/actions/workflows/ci.yml/badge.svg)](https://github.com/fireflylabss/abstract/actions/workflows/ci.yml)
[![latest release](https://img.shields.io/github/v/release/fireflylabss/abstract)](https://github.com/fireflylabss/abstract/releases)
[![license](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

The `rune` branch is the staged Rune rewrite. The screenshots and packaged releases
below show v0.1.5; the Rune build currently provides the Phase 1 skeleton and
Phase 2a editing behaviours, Phase 2b tabs, Phase 2c find/search/links and Phase 3 settings described here.

![abstract v0.1.5, light theme](docs/screenshot-light.png)

![abstract, dark theme](docs/screenshot-dark.png)

Your notes are plain `.md` files living in real folders on disk. No accounts, no sync daemon, no proprietary format — the folder is the app.

## Phase 1

- **Live Markdown** — one native multiline text model, styled with tree-sitter.
  Heading and inline syntax reveals on the caret/selection's lines; the resulting
  text shift is intentional. Lists, tasks and quote markers keep their geometry.
  Tables remain raw monospace source. Rune supplies undo, clipboard, selection and IME.
- **Autosave** — 600 ms debounce, a single revision-ordered background writer,
  unique atomic temporary files, visible save failures, and synchronous quit flushing
  of every unacknowledged buffer, including failed writes. Quit errors are reported
  on stderr. Switching notes does not discard dirty buffers.
- **Sidebar** — expand folders, click notes, and create an empty, collision-safe note
  with **New note** or `Cmd/Ctrl+N`. `Cmd/Ctrl+S` retries/saves pending edits.
- **Active space** — reads the existing spaces file, or pass a note directory:
  `abstract /path/to/notes`. Generation tags prevent older scans replacing newer trees.
- **Appearance** — existing light/dark/system palettes, font, zoom, text width and
  English/Portuguese language preferences are read from the unchanged settings file.
  Noto Sans and Noto Sans Mono remain bundled.
- **Responsive analysis** — below 64 KiB, analysis runs synchronously once per edit;
  at or above 64 KiB, a background worker publishes only the current revision.
  Source remains editable while analysis catches up.

The pure logic lives in `src/core/`, persistence in `src/io/`, and Rune views in
`src/ui/`. Buffer, Markdown/code/footnote parsing, find, lines, smart editing, tables,
vault, search, links, spaces, preferences, store and i18n compile now; later phases
will wire their additional UI. HTML/PDF export, attachments, math rendering, spell,
watch, updates, crash reporting and Discord sources are retained in `core/` but
excluded from this build and their dependencies are not pulled in. Spell dictionary
compression will also need restoring when spell is wired.

## Phase 2a editing

- **Enter** continues bullets, numbered lists, unchecked tasks and quotes; an empty
  item exits the list. Fenced code keeps its indentation without continuing markers.
- **Tab / Shift+Tab** indents/outdents selected list lines by two spaces. Elsewhere,
  Tab inserts two spaces. `Cmd/Ctrl+Enter` toggles the current task.
- **Formatting** — `Cmd/Ctrl+B`, `I`, `E`, `Shift+X` and `K` wrap/toggle bold,
  italic, inline code, strikethrough and a Markdown link. Empty selections insert a
  pair with the caret inside. `Shift+H` highlights and `/` wraps hidden comments.
- **Editing** — `Alt+Up/Down` moves lines, `Cmd/Ctrl+Shift+D` duplicates lines,
  and `Cmd/Ctrl+D` selects the word/next occurrence. Each text command is one undo
  step; Rune still owns native text navigation, selection, clipboard and IME.
- **Appearance keys** — `Cmd/Ctrl` with `+`, `-`, `0` zooms in/out/resets;
  `Shift+L` cycles system/light/dark, `\` toggles the sidebar, `Shift+Enter` toggles
  focus mode, and Escape leaves focus mode. Theme/zoom persist through the ordered
  writer; sidebar visibility is restored from the session at startup.

Checkbox clicking is not wired: the pinned Rune text input does not expose a
cheap pointer-to-source-offset callback. Use `Cmd/Ctrl+Enter` instead.

## Phase 2b tabs

- Sidebar notes open in new tabs or focus the existing tab. The compact strip
  stays visible for a single tab, with titles, dirty dots and hover close buttons.
- Each note keeps its selection, scroll position, undo history and analysis cache.
  Inactive notes autosave; closing flushes edits and failed writes retain their buffer.
- `Cmd/Ctrl+T` or `N` creates a note; `Cmd/Ctrl+W` closes a tab. `Ctrl+Tab` and
  `Ctrl+Shift+Tab` cycle; `Cmd+Shift+]` / `[` and `Ctrl+PageDown` / `PageUp`
  also navigate. `Cmd/Ctrl+1..9` selects a tab (`9` selects the last).
- Open tab order, active tab, cursors and scroll positions use the existing session
  format. Missing files are skipped during background restore.

## Phase 3 settings and customization

`Cmd/Ctrl+,` opens a keyboard-accessible editor-area settings panel; Escape
returns to the note without touching its buffer or undo history. Appearance,
Editor and Files group the live controls. Tab moves between controls; Space
activates them. Changes use the ordered writer and retain unknown and
unported settings keys.

Choose palette/accent swatches (including the system accent), independent UI
and editor font families, editor size/line height, blank-paragraph spacing,
heading scale, text/sidebar widths, single-tab visibility, syntax concealment,
caret style/blinking, smart punctuation/pairing and autosave delay. Font presets
fall back through Rune when unavailable. The legacy `font` still supplies both
families until separately overridden. Smart pairing is off by default to keep
the previous editing behavior; it never transforms paste or IME commits.
Drag the sidebar divider to resize it. Existing zoom shortcuts scale the entire
window, including text, controls and pointer geometry, and persist their value.

Glass uses Rune's native macOS popover vibrancy behind translucent surface tints,
with the existing per-surface opacity/intensity and advanced tint/text settings.
Other platforms keep opaque surfaces where Rune has no native material backend.
Headless rendering shows the opaque fallback rather than desktop blur. Settings
for unported tables, tour, exports, spelling, updates and Discord remain hidden.

## Phase 2c find, search and links

- `Cmd/Ctrl+F` opens the one-row find bar; `Cmd/Ctrl+Alt+F` or `Cmd/Ctrl+H`
  adds replacement. Matches update while typing, with a current/total count,
  source highlights and an **Aa** case-sensitive toggle.
- `Enter` / `Shift+Enter`, `Cmd/Ctrl+G` / `Shift+G` and `F3` / `Shift+F3`
  move between matches. In the replacement field, `Enter` replaces one and
  `Cmd/Ctrl+Enter` replaces all. Each replacement command is one undo step.
  Escape closes the panel and returns focus to the editor.
- `Cmd/Ctrl+Shift+F` searches all notes in the active space on a worker, including
  unsaved open buffers. Up/Down selects a result; Enter or a click opens its original
  byte range. Stale query generations cannot replace newer results.
- **Alt+Enter** follows the link under the caret: wiki-links resolve titles/stems
  case-insensitively or create a missing note, local `.md` links open in tabs,
  heading fragments jump within notes, and HTTP(S) links open in the browser.
  Wiki aliases are supported. Pointer-following is not wired because the pinned
  Rune text input has no cheap pointer-to-source-offset callback.

Images,
external-edit watching, rename/move/trash, exports, command
palette and updater are not yet available. Settings/session/spaces file formats
remain compatible; window geometry restoration remains deferred.

## Install

### Releases

Prebuilt artifacts are attached to each [GitHub Release](https://github.com/fireflylabss/abstract/releases):

- **Linux** — `abstract-<version>-linux-x86_64.tar.gz` (binary + docs), a `.deb`, an `.rpm`, and an `.AppImage`. aarch64 builds are available as `abstract-<version>-linux-aarch64.*`.
- **macOS** — `abstract-<version>-macos-aarch64.dmg` (Apple Silicon) / `...-macos-x86_64.dmg` (Intel): open and drag `abstract.app` to Applications. A bare `.app.zip` is also attached. Builds are ad-hoc signed (no Apple Developer ID), so Gatekeeper blocks the first launch: right-click → **Open**, or run `xattr -dr com.apple.quarantine /Applications/abstract.app`.
- **Windows** — `abstract-<version>-windows-x86_64.exe` (portable, just run it) or the `.zip` with README/LICENSE. SmartScreen may warn about an unknown publisher: **More info → Run anyway**.

### Updates

Automatic updates are not wired in the Rune Phase 1 build. Packaged v0.1.5 releases
retain their previous updater; update package-managed installs with your package manager.

### Arch / CachyOS (AUR)

Once published to the AUR (PKGBUILDs in [`packaging/aur/`](packaging/aur)):

```bash
paru -S abstract-editor      # build from source
paru -S abstract-editor-bin  # prebuilt binary
```

### Debian / Ubuntu

Download the `.deb` from the latest release, then:

```bash
sudo dpkg -i abstract-editor_<version>-1_amd64.deb
```

### Fedora / RHEL

Download the `.rpm` from the latest release, then:

```bash
sudo dnf install ./abstract-editor-<version>-1.x86_64.rpm
```

### AppImage

```bash
chmod +x abstract-<version>-linux-x86_64.AppImage
./abstract-<version>-linux-x86_64.AppImage
```

### From source

Rust **1.99.1** is pinned via `rust-toolchain.toml` (rustup handles it automatically).
Rune and rune-kit, plus the required winit, parley_engine, wgpu-hal and AccessKit
patches, are pinned to git revision **33eabc2**. Keep all pins together; an arbitrary
Rune development HEAD is not a supported substitute. Builds need git access to
GitHub on the first run. This app is not published to crates.io.

- **macOS** — `xcode-select --install`
- **Arch** —
  ```bash
  sudo pacman -S --needed rustup base-devel pkgconf libxkbcommon libxkbcommon-x11 \
    libxcb xcb-util-wm xcb-util-image xcb-util-keysyms xcb-util-renderutil \
    wayland alsa-lib fontconfig
  ```
- **Debian/Ubuntu** —
  ```bash
  sudo apt-get install libxcb-xkb-dev libxkbcommon-dev libxkbcommon-x11-dev \
    libxcb-icccm4-dev libxcb-image0-dev libxcb-render0-dev libxcb-shape0-dev \
    libxcb-xfixes0-dev libxcb-keysyms1-dev libwayland-dev libfontconfig-dev pkg-config
  ```
- **Windows** — [rustup](https://rustup.rs) + Visual Studio Build Tools ("Desktop development with C++" workload).

Then:

```bash
cargo build --release
./target/release/abstract   # abstract.exe on Windows
```

## Keyboard

`Ctrl` on Linux/Windows, `Cmd` on macOS. Standard editing keys plus `B`/`I`/`Z`/`Shift+Z` and word jumps (`Ctrl+←/→`, `Alt+←/→` on macOS).

| Action | Linux / Windows | macOS |
| --- | --- | --- |
| New note | `Ctrl+N` | `Cmd+N` |
| New folder | `Ctrl+Shift+N` | `Cmd+Shift+N` |
| Switch space | `Ctrl+O` | `Cmd+O` |
| Save now | `Ctrl+S` | `Cmd+S` |
| Cycle theme | `Ctrl+Shift+L` | `Cmd+Shift+L` |
| Settings | `Ctrl+,` | `Cmd+,` |
| Toggle sidebar | `Ctrl+\` | `Cmd+\` |
| Delete note | `Ctrl+Shift+Backspace` | `Cmd+Shift+Backspace` |
| Search notes | `Ctrl+P` or `Ctrl+Shift+F` | `Cmd+P` or `Cmd+Shift+F` |
| Find in note | `Ctrl+F` | `Cmd+F` |
| Replace in note | `Ctrl+H` | `Cmd+Alt+F` |
| Next / previous match | `F3` / `Shift+F3` | `Cmd+G` / `Cmd+Shift+G` |
| Move line(s) up / down | `Alt+↑` / `Alt+↓` | `Alt+↑` / `Alt+↓` |
| Duplicate line(s) | `Ctrl+Shift+D` | `Cmd+Shift+D` |
| Select next occurrence | `Ctrl+D` | `Cmd+D` |
| Rename | `F2` | `F2` |
| Tour | `F1` | `F1` |
| Quit | — | `Cmd+Q` |

Tour navigation: `Enter`/`→` next, `←` back, `Esc` skip.

Find bar: `Enter`/`Shift+Enter` step through matches. In the replace field `Enter` replaces the active match and `Ctrl+Enter` (`Cmd+Enter`) replaces all of them as one undo step. `Esc` closes.

Tables render as a grid; with the caret inside one (or with Settings > Raw tables on) you edit its Markdown source. In a table, `Tab` / `Shift+Tab` realign the columns and move to the next / previous cell; `Tab` in the last cell adds a row.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

## License

Apache-2.0 — see [LICENSE](LICENSE). Bundled Noto Sans / Noto Sans Mono fonts are licensed under the SIL Open Font License 1.1 — see [assets/fonts/OFL.txt](assets/fonts/OFL.txt). Bundled Hunspell dictionaries: `en_US` under MIT AND BSD, `pt_BR` under LGPL-3.0 OR MPL-2.0 (distributed under MPL-2.0) — see [assets/dict/](assets/dict/).
