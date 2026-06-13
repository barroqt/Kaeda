use std::path::{Path, PathBuf};
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
    #[error("card with id {0} not found")]
    CardNotFound(u32),
}

#[derive(Debug, Clone)]
pub enum SubtitleSource {
    ExternalSrt {
        srt_path: PathBuf,
        video_path: Option<PathBuf>,
    },
    Embedded {
        video_path: PathBuf,
    },
}

#[derive(Debug, Error)]
pub enum ExtractError {
    #[error("unsupported subtitle source: embedded subtitles not yet implemented")]
    UnsupportedForNow,
    #[error("failed to read subtitle file: {0}")]
    Io(#[from] std::io::Error),
    #[error("subtitle parsing error: {0}")]
    Parse(String),
}

pub fn prepare_session_subtitles(
    source: SubtitleSource,
) -> Result<Vec<SubtitleEntry>, ExtractError> {
    match source {
        SubtitleSource::ExternalSrt { srt_path, .. } => {
            entries_from_srt(&srt_path).map_err(|e| match e {
                CoreError::Io(io) => ExtractError::Io(io),
                other => ExtractError::Parse(other.to_string()),
            })
        }
        SubtitleSource::Embedded { .. } => Err(ExtractError::UnsupportedForNow),
    }
}

pub fn entries_from_srt(path: &Path) -> Result<Vec<SubtitleEntry>, CoreError> {
    let content = std::fs::read_to_string(path)?;
    let content = content.replace("\r\n", "\n").replace("\r", "\n");
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

pub fn srt_timestamp_to_ms(timestamp: &str) -> Option<u64> {
    let ts = timestamp.trim();
    let (time_part, millis_part) = ts.rsplit_once(',')?;
    let millis: u64 = millis_part.parse().ok()?;
    let mut parts = time_part.split(':');
    let hours: u64 = parts.next()?.parse().ok()?;
    let minutes: u64 = parts.next()?.parse().ok()?;
    let seconds: u64 = parts.next()?.parse().ok()?;
    Some(hours * 3_600_000 + minutes * 60_000 + seconds * 1_000 + millis)
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
    fn srt_timestamp_to_ms_returns_correct_milliseconds() {
        assert_eq!(srt_timestamp_to_ms("00:00:01,000"), Some(1000));
        assert_eq!(srt_timestamp_to_ms("00:00:04,000"), Some(4000));
        assert_eq!(srt_timestamp_to_ms("00:00:09,500"), Some(9500));
        assert_eq!(srt_timestamp_to_ms("00:00:12,300"), Some(12300));
        assert_eq!(srt_timestamp_to_ms("00:01:00,000"), Some(60_000));
        assert_eq!(srt_timestamp_to_ms("01:00:00,000"), Some(3_600_000));
        assert_eq!(srt_timestamp_to_ms(""), None);
        assert_eq!(srt_timestamp_to_ms("invalid"), None);
    }

    #[test]
    fn parse_empty_file_returns_empty() {
        let path = fixture_path("empty.srt");
        let entries = entries_from_srt(&path).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn prepare_session_subtitles_external_srt_returns_correct_count() {
        let path = fixture_path("sample.srt");
        let source = SubtitleSource::ExternalSrt {
            srt_path: path,
            video_path: None,
        };
        let entries = prepare_session_subtitles(source).unwrap();
        assert_eq!(entries.len(), 5);
    }

    #[test]
    fn prepare_session_subtitles_preserves_timestamps() {
        let path = fixture_path("sample.srt");
        let source = SubtitleSource::ExternalSrt {
            srt_path: path,
            video_path: None,
        };
        let entries = prepare_session_subtitles(source).unwrap();
        assert_eq!(entries[0].start_time, "00:00:01,000");
        assert_eq!(entries[0].end_time, "00:00:04,000");
    }

    #[test]
    fn prepare_session_subtitles_strips_html_tags() {
        let path = fixture_path("sample.srt");
        let source = SubtitleSource::ExternalSrt {
            srt_path: path,
            video_path: None,
        };
        let entries = prepare_session_subtitles(source).unwrap();
        assert_eq!(entries[0].text, "안녕하세요 반갑습니다.");
    }

    #[test]
    fn prepare_session_subtitles_skips_malformed_blocks() {
        let path = fixture_path("sample_malformed.srt");
        let source = SubtitleSource::ExternalSrt {
            srt_path: path,
            video_path: None,
        };
        let entries = prepare_session_subtitles(source).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, 1);
        assert_eq!(entries[1].id, 4);
    }

    #[test]
    fn prepare_session_subtitles_empty_file_returns_empty() {
        let path = fixture_path("empty.srt");
        let source = SubtitleSource::ExternalSrt {
            srt_path: path,
            video_path: None,
        };
        let entries = prepare_session_subtitles(source).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn parse_handles_crlf_line_endings() {
        use std::io::Write;
        let dir = std::env::temp_dir();
        let path = dir.join("kaeda_test_crlf.srt");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "1\r\n00:00:01,000 --> 00:00:04,000\r\nHello\r\n\r\n2\r\n00:00:05,000 --> 00:00:08,000\r\nWorld\r\n").unwrap();
        drop(f);

        let entries = entries_from_srt(&path).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, 1);
        assert_eq!(entries[0].text, "Hello");
        assert_eq!(entries[1].text, "World");

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn prepare_session_subtitles_embedded_returns_unsupported() {
        let source = SubtitleSource::Embedded {
            video_path: PathBuf::from("/videos/test.mp4"),
        };
        let result = prepare_session_subtitles(source);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ExtractError::UnsupportedForNow
        ));
    }

    #[test]
    fn prepare_session_subtitles_missing_file_returns_io_error() {
        let source = SubtitleSource::ExternalSrt {
            srt_path: PathBuf::from("/nonexistent/file.srt"),
            video_path: None,
        };
        let result = prepare_session_subtitles(source);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ExtractError::Io(_)));
    }
}
