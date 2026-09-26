# ◇ abstract

[![ci](https://github.com/horizzon3507/abstract-editor/actions/workflows/ci.yml/badge.svg)](https://github.com/horizzon3507/abstract-editor/actions/workflows/ci.yml)

**abstract** — a minimal markdown notes editor, GPU-rendered with [GPUI](https://gpui.rs).

Local-first: your notes are plain `.md` files living in real folders on disk. No accounts, no sync daemon, no proprietary format — the folder is the app.

## Features

- **Live markdown** — Typora-style rendering powered by tree-sitter: bold, italic, code, links and headings render inline, and the syntax conceals itself until your selection touches it.
- **Autosave** — writes are debounced as you type; a note's file is created on the first keystroke and named after its first heading.
- **Spaces** — keep several note directories and switch between them from the sidebar.
- **Sidebar tree** — folders and notes with inline rename, plus one-click new note / new folder.
- **External-edit aware** — detects files changed on disk while open (mtime-based), so a pending save never silently clobbers outside edits.
- **Session restore** — reopens your notes, window geometry and sidebar state where you left off.
- **Monochrome themes** — light/dark cycling, tuned for writing.
- **First-run tour** — coach marks introduce the interface.
- **Chromeless** — custom titlebar and window controls; nothing between you and the text.

## Keyboard

Standard editing keys, `Ctrl` + arrows for word jumps, `Ctrl+B` bold, `Ctrl+I` italic, `Ctrl+Z`/`Ctrl+Shift+Z` undo/redo. Tour: `Enter`/`→` next, `←` back, `Esc` skip.

Shortcuts are `Ctrl` on Linux and `Cmd` on macOS: `Ctrl/Cmd+N` new note, `Ctrl/Cmd+Shift+N` new folder, `Ctrl/Cmd+O` switch space, `Ctrl/Cmd+S` save now, `Ctrl/Cmd+Shift+L` cycle theme, `Ctrl/Cmd+\` toggle sidebar, `Ctrl/Cmd+Shift+Backspace` delete note, `F2` rename, `F1` tour, `Cmd+Q` quit (macOS).

## Install

Requires Rust (pinned toolchain via `rust-toolchain.toml`). Supported platforms: Linux (Wayland or X11) and macOS.

```bash
cargo run --release
```

Or install the binary:

```bash
cargo install --path .
abstract-editor
```

### Releases

Prebuilt artifacts are attached to each [GitHub Release](https://github.com/horizzon3507/abstract-editor/releases):

- `abstract-editor-<version>-linux-x86_64.tar.gz` and a `.deb` package for Debian/Ubuntu.
- `abstract-<version>-macos-aarch64.app.zip` / `...-macos-x86_64.app.zip` — bare `abstract.app` bundles plus a raw-binary tarball. The app is ad-hoc signed, so on first launch either right-click → **Open**, or run `xattr -dr com.apple.quarantine abstract.app`.

### macOS

Install the Xcode Command Line Tools — no other system dependencies:

```bash
xcode-select --install
```

### Linux

System dependencies (Debian/Ubuntu):

```bash
sudo apt-get install libxcb-xkb-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libxcb-icccm4-dev libxcb-image0-dev libxcb-render0-dev libxcb-shape0-dev \
  libxcb-xfixes0-dev libxcb-keysyms1-dev libwayland-dev pkg-config
```

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

## License

Apache-2.0 — see [LICENSE](LICENSE).
