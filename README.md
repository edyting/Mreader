# Mreader

**Mreader** is a native desktop application for practising faster, more consistent reading with a configurable visual pacer.

It is built with **Rust** and **GTK4** (gtk-rs), works offline, and supports Linux and Windows.

## Features

- Open **PDF**, **DOCX**, **EPUB**, **TXT**, **Markdown**, **HTML**, and **RTF**
- Focus Reading View with reflowed text and PDF Original View
- Five pacing modes: Word Highlight, Moving Underline, Reading Pointer, Phrase Pacing, and RSVP
- Adjustable speed from **50–1000 WPM**, with presets and a live slider
- Search, paragraph navigation, bookmarks, saved progress, history, and local SQLite statistics
- Light, Dark, and Sepia themes
- Documents and reading activity remain on your device

## Build on Linux

Requirements: Rust stable, `pkg-config`, GTK4 development libraries, and Poppler GLib development libraries.

On Ubuntu/Debian:

```bash
sudo apt install build-essential pkg-config libgtk-4-dev libpoppler-glib-dev libsqlite3-dev
cargo build --release
cargo test
```

Run the app with:

```bash
./target/release/mreader
```

## Windows build

The Windows app uses GTK4 and Poppler from MSYS2/MinGW-w64. Install Rust stable with the `x86_64-pc-windows-gnu` target, then install these packages in the MSYS2 MINGW64 shell:

```bash
pacman -S --needed mingw-w64-x86_64-toolchain mingw-w64-x86_64-gtk4 \
  mingw-w64-x86_64-poppler mingw-w64-x86_64-pkgconf \
  mingw-w64-x86_64-adwaita-icon-theme zip
rustup target add x86_64-pc-windows-gnu
bash scripts/build-windows.sh
```

The script creates `target/mreader-windows-x64.zip`, a portable ZIP containing `mreader.exe` and its GTK/Poppler runtime files. Extract it and run `mreader.cmd`. The executable by itself needs the runtime DLLs in the same folder; use the portable bundle to run the app.

GitHub Actions also builds both artifacts on pushes to `main`, version tags, pull requests, or manual runs. Download `mreader-windows-x64-exe` for the executable or `mreader-windows-x64` for the complete portable app from the workflow run.

## Debian package

```bash
cargo install cargo-deb
cargo deb
sudo apt install ./target/debian/mreader_1.0.0-1_amd64.deb
```

Remove with `sudo apt remove mreader`.

## Keyboard shortcuts

| Shortcut | Action |
|----------|--------|
| Space | Play / Pause |
| ← / → | Previous / Next word (or PDF page in Original View) |
| ↑ / ↓ | Increase / Decrease speed |
| Esc | Stop pacing |
| Ctrl+O | Open document |
| Ctrl+B | Bookmark position |
| Ctrl++ / Ctrl+- | Font size |

## Data migration

On first launch, Mreader reuses existing RustPace settings and reading statistics when found. New settings and database files are stored under the Mreader application data folders.

## Licence

GPL-3.0-or-later — see [LICENSE](LICENSE).
