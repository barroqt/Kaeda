# Kaeda Quick Start

From video to Anki in minutes.

---

## 1. Install Kaeda

**macOS** — Download the `.tar.gz` for your chip (Apple Silicon or Intel), extract, and drag `Kaeda.app` to Applications.

**Windows** — Download the `.tar.gz`, extract, and run `Kaeda.exe`.

**Linux** — Download the `.tar.gz`, extract, and run `./kaeda`.

All downloads are available at [kaeda.app](https://kaeda.app).

---

## 2. Open a video and subtitle file

1. Click **Open Video** and select an `.mp4` or `.mkv` file.
2. Click **Open Subtitles** and select the matching `.srt` file.
3. The subtitle list appears on the left. The video appears in the center.

> If your video has embedded subtitles, you can use **File → Open Video with Embedded Subtitles** instead.

---

## 3. Navigate subtitles and select lines

Use the keyboard or mouse to move through subtitles:

| Shortcut | Action |
|----------|--------|
| `W` / `S` | Previous / next subtitle |
| `A` / `D` | Previous / next word (token) |
| `Space` | Play / pause video |
| `K` | Mark the current line as known |
| `R` | Replay current subtitle audio |

Clicking a subtitle in the list seeks the video to that line and makes it the current selection.

---

## 4. Create cards

1. Navigate to a subtitle you want to mine.
2. Click a word (token) in the sentence to select it as the **target**.
3. A dictionary lookup runs automatically — the definition appears in the card preview panel.
4. Edit the explanation or add notes if desired.
5. Press **Cmd+Enter** (Mac) / **Ctrl+Enter** (Windows/Linux) or click **Save Card**.

Repeat for each line you want to add to your deck.

---

## 5. Export your deck

1. Click **Export** (or use the menu **File → Export Session**).
2. Choose a location and filename. A `.tsv` file is written.

The TSV uses three tab-separated columns: `Target`, `Sentence`, `Explanation`.

---

## 6. Import into Anki

1. Open Anki.
2. Go to **File → Import**.
3. Select the exported `.tsv` file.
4. Set the field mapping:
   - Field 1 → `Target` (or your word field)
   - Field 2 → `Sentence` (or your expression field)
   - Field 3 → `Explanation` (or your meaning field)
5. Set the deck and note type, then click **Import**.

---

## Keyboard shortcuts reference

| Key | Action |
|-----|--------|
| `W` / `S` | Previous / next subtitle |
| `A` / `D` | Previous / next word token |
| `Space` | Toggle video play / pause |
| `←` / `→` | Seek video back / forward 15 s |
| `↑` / `↓` | Volume up / down |
| `M` | Toggle mute |
| `R` | Replay current line |
| `K` | Mark line known |
| `Cmd/Ctrl+Enter` | Save card |
| `Cmd/Ctrl+F` | Search subtitles |
| `Esc` | Clear search |

---

## Tips

- **Auto-translation**: Set up a DeepL API key in Settings to get automatic translations for each subtitle span.
- **Known lines**: Marking a line as known dims it in the subtitle list and hides it in future sessions for the same file.
- **Multiple decks**: Create and switch between decks from the deck selector in the toolbar.
