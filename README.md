# Kaeda

A **Korean vocabulary mining TUI** that parses `.srt` subtitle files, tokenizes the text, looks up definitions, and helps you build an SRS deck, all from the terminal.

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

## Data

All data lives in `.srt-miner/` at the project root — SQLite database, dictionary index, frequency list, and known words list.
