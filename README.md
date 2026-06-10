# Kaeda

A **Korean vocabulary mining TUI** that parses `.srt` subtitle files, tokenizes the text, looks up definitions, and helps you build an SRS deck, all from the terminal.

## Project structure

```
kaeda/
├── core/          # shared library — SRT parsing, tokenizer, dictionary API, store
├── app/src-tauri/ # future Tauri desktop app (scaffold)
├── src/           # root binary — CLI + TUI (mine, stats, known commands)
└── tests/         # test fixtures
```

The `kaeda-core` library contains all domain logic (parsing, tokenization, dictionary lookups, SQLite persistence). The root binary depends on it and adds the ratatui-based TUI and clap CLI interface.

## Quick start

```bash
cargo build
cargo run -- mine tests/fixtures/sample.srt
```

## Usage

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
| `Tab`     | Cycle active pane   |
| `a`       | Add word to deck    |
| `k`       | Mark word as known  |
| `s`       | Skip subtitle       |
| `q`       | Quit                |

The interface shows three panes: **context** (current subtitle), **candidates** (tokenized words), and **definitions** (dictionary lookup).

## Build & run

### CLI / TUI (root binary)

```bash
cargo build                    # builds root binary + core
cargo run -- mine <file.srt>   # start TUI mining session
cargo run -- stats             # show stats
cargo run -- known add <word>  # add known word
cargo run -- known list        # list known words
```

### Core library only

```bash
cargo build -p kaeda-core
cargo test -p kaeda-core
```

### Tauri desktop app (scaffold)

```bash
cargo build -p kaeda-app
cargo run -p kaeda-app
```

## Data

All data lives in `.srt-miner/` at the project root — SQLite database, dictionary index, frequency list, and known words list.
