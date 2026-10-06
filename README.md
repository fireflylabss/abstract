# <img src="assets/logo.png" width="96" alt="abstract logo"> abstract

**A minimal, local-first markdown notes editor — GPU-rendered with [GPUI](https://gpui.rs).**

[![ci](https://github.com/fireflylabss/abstract/actions/workflows/ci.yml/badge.svg)](https://github.com/fireflylabss/abstract/actions/workflows/ci.yml)
[![latest release](https://img.shields.io/github/v/release/fireflylabss/abstract)](https://github.com/fireflylabss/abstract/releases)
[![license](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

![abstract, light theme](docs/screenshot-light.png)

![abstract, dark theme](docs/screenshot-dark.png)

Your notes are plain `.md` files living in real folders on disk. No accounts, no sync daemon, no proprietary format — the folder is the app.

## Features

- **Live markdown** — Typora-style rendering powered by tree-sitter: bold, italic, `==highlights==`, `%%comments%%`, code, links, headings, lists, tasks, `> [!note]` callouts and `[^1]` footnotes render inline, and the syntax conceals itself until your selection touches it.
- **Autosave** — writes are debounced as you type; a note's file is created on the first keystroke and named after its first heading.
- **Spaces** — keep several note directories and switch between them from the sidebar.
- **Sidebar tree** — folders and notes with inline rename, drag-and-drop moves and a right-click menu, plus one-click new note / new folder.
- **Images and attachments** — paste or drop images onto a note: they are copied to `attachments/` and render inline, as do hand-written `![](…)` / `![[…]]` images. Other dropped files become links (`[[note]]` for notes in the space).
- **Context menus** — right-click in the editor for clipboard, links, formatting, paragraph styles and inserts.
- **Task lists** — click the checkbox to toggle; **code blocks** with syntax highlighting.
- **Wiki-links** — `[[note]]` links with autocomplete; `Ctrl`/`Cmd`+click follows a link (creating the note if missing), and a backlinks panel lists references.
- **Global search** — `Ctrl`/`Cmd`+`P` opens a palette over note titles and bodies; empty query lists recently edited notes.
- **External-edit aware** — the space folder is watched live (inotify/FSEvents/ReadDirectoryChanges), so edits and deletions from other apps show up instantly.
- **Session restore** — reopens your notes, window geometry and sidebar state.
- **Cross-platform** — Linux (Wayland & X11), macOS and Windows; bundled Noto fonts for consistent rendering.
- **Interface in English and Portuguese** — auto-detected from the system locale.
- **Chromeless** — custom titlebar, light and dark themes (Abstract, Paper, Sepia, Solarized, Midnight, Nord), first-run tour.

## Install

### Releases

Prebuilt artifacts are attached to each [GitHub Release](https://github.com/fireflylabss/abstract/releases):

- **Linux** — `abstract-<version>-linux-x86_64.tar.gz` (binary + docs), a `.deb`, an `.rpm`, and an `.AppImage`. aarch64 builds are available as `abstract-<version>-linux-aarch64.*`.
- **macOS** — `abstract-<version>-macos-aarch64.dmg` (Apple Silicon) / `...-macos-x86_64.dmg` (Intel): open and drag `abstract.app` to Applications. A bare `.app.zip` is also attached. Builds are ad-hoc signed (no Apple Developer ID), so Gatekeeper blocks the first launch: right-click → **Open**, or run `xattr -dr com.apple.quarantine /Applications/abstract.app`.
- **Windows** — `abstract-<version>-windows-x86_64.exe` (portable, just run it) or the `.zip` with README/LICENSE. SmartScreen may warn about an unknown publisher: **More info → Run anyway**.

### Updates

Once a day abstract checks GitHub for a newer release. Turn this off with **Check for updates** in the spaces menu. The AppImage, the macOS `.app`, the Windows `.exe` and the Linux tarball can update themselves. The download is only installed if `SHA256SUMS` carries a valid minisign signature and the file matches its checksum. `.deb`, `.rpm`, AUR and Homebrew installs just get a notice, so update them with your package manager.

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

Rust toolchain is pinned via `rust-toolchain.toml` (rustup handles it automatically).

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
    libxcb-xfixes0-dev libxcb-keysyms1-dev libwayland-dev pkg-config
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
