# Kaeda

A tool that helps language learner save their content into flashcards.

- **Kaeda** (Tauri desktop app) — video + SRT playback, card preview, session management, and
  Anki-compatible TSV export.
- **srt-miner** (CLI / TUI) — parse `.srt` subtitle files from a terminal, tokenize Korean text, look up definitions, and build an SRS deck.

## Kaeda desktop app

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 18+ and [pnpm](https://pnpm.io/installation)
- [Tauri system dependencies](https://v2.tauri.app/start/prerequisites/) for
  your platform (macOS: Xcode CLI tools; Linux: `libwebkit2gtk-4.1-dev` etc.)

### Run

```bash
# install JS dependencies
cd app && pnpm install

# run in development mode
cd app && cargo tauri dev
```

### Workflow

1. Click **Start Session**, pick a media and an `.srt` file
2. Navigate subtitles with **W/S** or mouse
3. Select words with **A/D** or click
4. The **Translation** field auto-fills from the Naver dictionary and is editable
5. **Save Card** (`⌘+Enter`) or **Mark as Known** (`K`)
6. Click **View Cards** to review, edit, or delete cards from the session and manage your decks
7. Click **Export TSV** to produce an Anki-importable file

If the video fails to load or play (unsupported codec, missing video server, etc.),
Kaeda shows a prominent fallback banner and dims the video area. The subtitle list
and card panel remain fully functional and you can still mine from subtitles, save
cards, and export.

### Keyboard shortcuts

| Key(s)                | Action                        |
| --------------------- | ----------------------------- |
| `W` / `S`             | Navigate subtitles (up / down)|
| `A` / `D`             | Select words (left / right)   |
| `K`                   | Mark current line as known    |
| `⌘+Enter` / `Ctrl+Enter` | Save card                  |
| Arrow keys + Space    | Video playback controls       |

## Release builds

### Cross-platform

```bash
./scripts/build_release.sh
```

This script:
1. Cleans previous artifacts (`dist/` and `target/release/kaeda*`).
2. Builds the CLI binary in release mode (`cargo build --release`).
3. Builds the Tauri desktop app (`cargo tauri build`).
4. Outputs versioned binaries and installers into `dist/kaeda-<version>-<target>/`.
5. Creates a compressed archive (`tar.gz` on macOS/Linux, `.zip` on Windows).

### Platform-specific

| OS        | Script                  |
| --------- | ----------------------- |
| macOS     | `./scripts/build_macos.sh` |
| Linux     | `./scripts/build_linux.sh` (EXPERIMENTAL) |
| Windows   | `scripts\build_windows.bat` |

> **Linux builds are experimental.** The app has not been extensively tested on
> Linux. You may encounter issues with WebKitGTK media codec support, missing
> system libraries, or window integration. Please report problems at
> [the issue tracker](https://github.com/barroqt/kaeda/issues).

### Verification

```bash
./scripts/test_release.sh [path/to/kaeda]
```

The test script runs the CLI binary through a battery of smoke tests:
`--help`, `stats`, `known add`/`list`, and parsing a sample `.srt` file.

## CLI / TUI

### Commands

| Command                  | Description                 |
| ------------------------ | --------------------------- |
| `kaeda mine <file.srt>`  | Start a mining session      |
| `kaeda stats`            | Show deck and session stats |
| `kaeda known add <word>` | Manually add a known word   |
| `kaeda known list`       | List all known words        |

### TUI controls (within `mine`)

| Key       | Action              |
| --------- | ------------------- |
| `↑` / `↓` | Navigate candidates |
| `←` / `→` | Navigate subtitles  |
| `a`       | Add word to deck    |
| `k`       | Mark word as known  |
| `s`       | Skip subtitle       |
| `q`       | Quit                |

The interface shows three panes: **context** (current subtitle),
**candidates** (tokenized words), and **definitions** (dictionary lookup).

### Build & run

```bash
cargo build                    # builds root binary + core
cargo run -- --version         # show version
cargo run -- mine <file.srt>   # start TUI mining session
cargo run -- stats             # show stats
cargo run -- known add <word>  # add known word
cargo run -- known list        # list known words
```

## Debugging

Backend logging uses the [`tracing`](https://docs.rs/tracing) crate. Set `RUST_LOG` to control verbosity:

| `RUST_LOG`      | Shows                                 |
| --------------- | ------------------------------------- |
| `info` (default)| config loads, saves, settings changes |
| `debug`         | + DeepL API call details, HTTP status |
| `error`         | failures only                         |

```bash
RUST_LOG=debug cargo tauri dev   # full trace
RUST_LOG=info  cargo tauri dev   # normal (default)
```
