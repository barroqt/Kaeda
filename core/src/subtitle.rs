use std::path::Path;
use thiserror::Error;

use crate::util::strip_html_tags;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubtitleEntry {
    pub id: u32,
    pub start_time: String,
    pub end_time: String,
    pub text: String,
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("failed to read subtitle file: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("tokenization error: {0}")]
    Tokenize(String),
    #[error("network request failed: {0}")]
    Network(String),
    #[error("failed to write export file: {0}")]
    Export(String),
}

pub fn entries_from_srt(path: &Path) -> Result<Vec<SubtitleEntry>, CoreError> {
    let content = std::fs::read_to_string(path)?;
    let mut subtitles: Vec<SubtitleEntry> = Vec::new();

    for block in content.split("\n\n") {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }

        let mut lines = block.lines();

        let Some(id) = lines.next().and_then(|l| l.parse::<u32>().ok()) else {
            continue;
        };
        let Some(timestamp_line) = lines.next() else {
            continue;
        };

        let (start_time, end_time) = match parse_timestamp_line(timestamp_line) {
            Some((s, e)) => (s, e),
            None => continue,
        };

        let text = strip_html_tags(&lines.collect::<Vec<_>>().join("\n"));
        if text.trim().is_empty() {
            continue;
        }

        subtitles.push(SubtitleEntry {
            id,
            start_time,
            end_time,
            text,
        });
    }

    Ok(subtitles)
}

fn parse_timestamp_line(line: &str) -> Option<(String, String)> {
    let mut parts = line.split(" --> ");
    let start = parts.next()?.trim().to_string();
    let end = parts.next()?.trim().to_string();
    Some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_path(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    #[test]
    fn parse_returns_correct_count() {
        let path = fixture_path("sample.srt");
        let entries = entries_from_srt(&path).unwrap();
        assert_eq!(entries.len(), 5);
    }

    #[test]
    fn parse_preserves_timestamps() {
        let path = fixture_path("sample.srt");
        let entries = entries_from_srt(&path).unwrap();
        assert_eq!(entries[0].start_time, "00:00:01,000");
        assert_eq!(entries[0].end_time, "00:00:04,000");
    }

    #[test]
    fn parse_strips_html_tags() {
        let path = fixture_path("sample.srt");
        let entries = entries_from_srt(&path).unwrap();
        assert_eq!(entries[0].text, "안녕하세요 반갑습니다.");
    }

    #[test]
    fn parse_skips_malformed_blocks() {
        let path = fixture_path("sample_malformed.srt");
        let entries = entries_from_srt(&path).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, 1);
        assert_eq!(entries[1].id, 4);
    }

    #[test]
    fn parse_empty_file_returns_empty() {
        let path = fixture_path("empty.srt");
        let entries = entries_from_srt(&path).unwrap();
        assert!(entries.is_empty());
    }
}
