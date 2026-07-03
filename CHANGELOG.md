# Changelog

All notable changes to Kaeda are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.0] — TBD

### Added

- **Core mining workflow** — Load a local video file (mp4/mkv) and external SRT subtitle file, navigate subtitles, select words, save cards to a session.
- **Korean tokenization** — Automatic morphological analysis powered by `kaeda-core`; click any word in a subtitle to select it as the card target.
- **Dictionary lookup** — Auto-fills card explanations from a built-in Korean dictionary (English definitions for common words and lemmas).
- **Card management** — Create, edit, and delete cards within a session. Cards store sentence, target word, explanation, deck association, and tags.
- **Deck system** — Create and switch between multiple decks. Each session belongs to one deck.
- **TSV export** — One-click export of all session cards in tab-separated format ready for Anki import.
- **Known-line tracking** — Mark subtitle lines as known; they are persisted per file and dimmed/hidden in future sessions.
- **Video playback** — Embedded HTML5 video player with custom controls (play/pause, seek, speed, volume, mute).
- **Subtitle search** — Search all subtitle lines by text with keyboard navigation through results.
- **Cross-platform builds** — Pre-built binaries for macOS (Apple Silicon + Intel), Windows, and Linux.
- **Translation span copy** — Copy a context window around the current subtitle (previous + current + next line).

### Infrastructure

- CI/CD via GitHub Actions (`release.yml`) producing `.tar.gz` archives per target.
- Website at [kaeda.app](https://kaeda.app) with download links, feature overview, and documentation.
