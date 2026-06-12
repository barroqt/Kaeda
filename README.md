# Kaeda

A tool that helps language learner save their content into flashcards.

- **srt-miner** (CLI / TUI) — parse `.srt` subtitle files from a terminal, tokenize Korean text,
  look up definitions, and build an SRS deck.
- **Kaeda** (Tauri desktop app) — the same mining workflow with a graphical
  interface: video + SRT playback, card preview, session management, and
  Anki-compatible TSV export.

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
cargo run -- mine <file.srt>   # start TUI mining session
cargo run -- stats             # show stats
cargo run -- known add <word>  # add known word
cargo run -- known list        # list known words
```

## Kaeda desktop app

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 18+ and [pnpm](https://pnpm.io/installation)
- [Tauri system dependencies](https://v2.tauri.app/start/prerequisites/) for
  your platform (macOS: Xcode CLI tools; Linux: `libwebkit2gtk-4.1-dev` etc.)

### Build & run

```bash
# install JS dependencies
cd app && pnpm install

# run in development mode
cd app && cargo tauri dev
```

### Workflow

1. Click **Start Session** and pick an `.srt` file
2. Navigate subtitles with arrow keys or mouse
3. Click a token to select it as the **target word**
4. The **Translation** field auto-fills from the Naver dictionary (editable)
5. **Save Card** (`⌘+Enter`), **Skip** (`s`), or **Mark as Known** (`k`)
6. Click **View Cards** to review, edit, or delete cards from the session
7. Click **Export TSV** to produce an Anki-importable file
