use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::embedded_subtitles;
use crate::ffmpeg;
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
    #[error("failed to open video file: {0}")]
    Open(String),
    #[error("no text subtitle track found in video")]
    NoSubtitleTrack,
    #[error("subtitle extraction failed: {0}")]
    Extraction(String),
    #[error("failed to read subtitle file: {0}")]
    Io(#[from] std::io::Error),
    #[error("subtitle parsing error: {0}")]
    Parse(String),
    #[error("ffmpeg extraction failed: {source}")]
    FfmpegFailed { source: ffmpeg::FfmpegExtractError },
    #[error("embedded subtitle extraction failed: {source}")]
    EmbeddedExtractionFailed {
        source: embedded_subtitles::SubtitleExtractError,
    },
}

pub fn prepare_session_subtitles(
    source: SubtitleSource,
) -> Result<Vec<SubtitleEntry>, ExtractError> {
    prepare_session_subtitles_impl(source, Some("ffmpeg"))
}

/// Implementation helper that accepts the ffmpeg binary name so tests can
/// control whether the ffmpeg fallback is attempted.
pub(crate) fn prepare_session_subtitles_impl(
    source: SubtitleSource,
    ffmpeg_binary: Option<&str>,
) -> Result<Vec<SubtitleEntry>, ExtractError> {
    let parse_and_cleanup = |srt_path: &Path| -> Result<Vec<SubtitleEntry>, ExtractError> {
        let entries = entries_from_srt(srt_path).map_err(|e| match e {
            CoreError::Io(io) => ExtractError::Io(io),
            other => ExtractError::Parse(other.to_string()),
        })?;
        let _ = std::fs::remove_file(srt_path);
        Ok(entries)
    };

    match source {
        SubtitleSource::ExternalSrt { srt_path, .. } => {
            entries_from_srt(&srt_path).map_err(|e| match e {
                CoreError::Io(io) => ExtractError::Io(io),
                other => ExtractError::Parse(other.to_string()),
            })
        }
        SubtitleSource::Embedded { video_path } => {
            match embedded_subtitles::extract_to_srt(&video_path) {
                Ok(srt_path) => parse_and_cleanup(&srt_path),
                Err(err) if err.is_retryable_with_ffmpeg() => {
                    let ffmpeg_ok = ffmpeg_binary
                        .map(ffmpeg::command_available)
                        .unwrap_or(false);
                    if ffmpeg_ok {
                        match ffmpeg::extract_with_ffmpeg_impl(
                            ffmpeg_binary.unwrap(),
                            &video_path,
                            None,
                            &std::env::temp_dir(),
                        ) {
                            Ok(srt_path) => parse_and_cleanup(&srt_path),
                            Err(ff_err) => Err(ExtractError::FfmpegFailed { source: ff_err }),
                        }
                    } else {
                        Err(ExtractError::EmbeddedExtractionFailed { source: err })
                    }
                }
                Err(err) => Err(ExtractError::EmbeddedExtractionFailed { source: err }),
            }
        }
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

fn ends_with_sentence_ending(s: &str) -> bool {
    s.trim().ends_with('.') || s.trim().ends_with('?') || s.trim().ends_with('!')
}

pub fn build_translation_span(subtitles: &[SubtitleEntry], current_index: usize) -> String {
    if current_index >= subtitles.len() {
        return String::new();
    }

    let mut parts = Vec::new();

    if current_index > 0 && !ends_with_sentence_ending(&subtitles[current_index - 1].text) {
        parts.push(subtitles[current_index - 1].text.as_str());
    }
    parts.push(subtitles[current_index].text.as_str());
    if current_index + 1 < subtitles.len()
        && !ends_with_sentence_ending(&subtitles[current_index].text)
    {
        parts.push(subtitles[current_index + 1].text.as_str());
    }

    parts.join("\n")
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
    fn prepare_session_subtitles_embedded_missing_file_without_ffmpeg_fallback() {
        let source = SubtitleSource::Embedded {
            video_path: PathBuf::from("/nonexistent/video.mkv"),
        };
        let result = prepare_session_subtitles_impl(source, None);
        assert!(result.is_err());
        assert!(
            matches!(
                result.unwrap_err(),
                ExtractError::EmbeddedExtractionFailed { .. }
            ),
            "expected EmbeddedExtractionFailed for missing video file without ffmpeg"
        );
    }

    #[test]
    fn prepare_session_subtitles_embedded_no_ffmpeg_binary_returns_embedded_error() {
        let source = SubtitleSource::Embedded {
            video_path: PathBuf::from("/nonexistent/video.mkv"),
        };
        let result = prepare_session_subtitles_impl(source, Some("nonexistent_ffmpeg_xyz"));
        assert!(result.is_err());
        assert!(
            matches!(
                result.unwrap_err(),
                ExtractError::EmbeddedExtractionFailed { .. }
            ),
            "expected EmbeddedExtractionFailed when ffmpeg binary does not exist"
        );
    }

    #[test]
    fn prepare_session_subtitles_external_srt_still_works_after_refactor() {
        let path = fixture_path("sample.srt");
        let source = SubtitleSource::ExternalSrt {
            srt_path: path,
            video_path: None,
        };
        let result = prepare_session_subtitles_impl(source, None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 5);
    }

    #[test]
    fn build_translation_span_middle_index_returns_prev_current_next() {
        let subtitles = vec![
            SubtitleEntry {
                id: 1,
                start_time: "00:00:01,000".into(),
                end_time: "00:00:04,000".into(),
                text: "첫 번째 자막".into(),
            },
            SubtitleEntry {
                id: 2,
                start_time: "00:00:05,000".into(),
                end_time: "00:00:08,000".into(),
                text: "두 번째 자막".into(),
            },
            SubtitleEntry {
                id: 3,
                start_time: "00:00:09,000".into(),
                end_time: "00:00:12,000".into(),
                text: "세 번째 자막".into(),
            },
        ];

        let span = build_translation_span(&subtitles, 1);
        assert_eq!(span, "첫 번째 자막\n두 번째 자막\n세 번째 자막");
    }

    #[test]
    fn build_translation_span_first_index_returns_current_and_next() {
        let subtitles = vec![
            SubtitleEntry {
                id: 1,
                start_time: "00:00:01,000".into(),
                end_time: "00:00:04,000".into(),
                text: "첫 번째 자막".into(),
            },
            SubtitleEntry {
                id: 2,
                start_time: "00:00:05,000".into(),
                end_time: "00:00:08,000".into(),
                text: "두 번째 자막".into(),
            },
        ];

        let span = build_translation_span(&subtitles, 0);
        assert_eq!(span, "첫 번째 자막\n두 번째 자막");
    }

    #[test]
    fn build_translation_span_last_index_returns_prev_and_current() {
        let subtitles = vec![
            SubtitleEntry {
                id: 1,
                start_time: "00:00:01,000".into(),
                end_time: "00:00:04,000".into(),
                text: "첫 번째 자막".into(),
            },
            SubtitleEntry {
                id: 2,
                start_time: "00:00:05,000".into(),
                end_time: "00:00:08,000".into(),
                text: "두 번째 자막".into(),
            },
        ];

        let span = build_translation_span(&subtitles, 1);
        assert_eq!(span, "첫 번째 자막\n두 번째 자막");
    }

    #[test]
    fn build_translation_span_single_line_returns_that_line() {
        let subtitles = vec![SubtitleEntry {
            id: 1,
            start_time: "00:00:01,000".into(),
            end_time: "00:00:04,000".into(),
            text: "유일한 자막".into(),
        }];

        let span = build_translation_span(&subtitles, 0);
        assert_eq!(span, "유일한 자막");
    }

    #[test]
    fn build_translation_span_out_of_bounds_returns_empty() {
        let subtitles = vec![SubtitleEntry {
            id: 1,
            start_time: "00:00:01,000".into(),
            end_time: "00:00:04,000".into(),
            text: "유일한 자막".into(),
        }];

        let span = build_translation_span(&subtitles, 5);
        assert_eq!(span, "");
    }

    #[test]
    fn build_translation_span_empty_list_returns_empty() {
        let subtitles: Vec<SubtitleEntry> = vec![];
        let span = build_translation_span(&subtitles, 0);
        assert_eq!(span, "");
    }

    #[test]
    fn build_translation_span_skips_prev_when_it_ends_with_period() {
        let subtitles = vec![
            SubtitleEntry {
                id: 1,
                start_time: "00:00:01,000".into(),
                end_time: "00:00:04,000".into(),
                text: "첫 번째 문장입니다.".into(),
            },
            SubtitleEntry {
                id: 2,
                start_time: "00:00:05,000".into(),
                end_time: "00:00:08,000".into(),
                text: "두 번째 문장입니다. 중간에".into(),
            },
        ];

        let span = build_translation_span(&subtitles, 1);
        assert_eq!(span, "두 번째 문장입니다. 중간에");
    }

    #[test]
    fn build_translation_span_skips_next_when_current_ends_with_period() {
        let subtitles = vec![
            SubtitleEntry {
                id: 1,
                start_time: "00:00:01,000".into(),
                end_time: "00:00:04,000".into(),
                text: "끝난 문장입니다.".into(),
            },
            SubtitleEntry {
                id: 2,
                start_time: "00:00:05,000".into(),
                end_time: "00:00:08,000".into(),
                text: "다음 자막입니다.".into(),
            },
        ];

        let span = build_translation_span(&subtitles, 0);
        assert_eq!(span, "끝난 문장입니다.");
    }

    #[test]
    fn build_translation_span_skips_both_when_both_are_sentence_endings() {
        let subtitles = vec![
            SubtitleEntry {
                id: 1,
                start_time: "00:00:01,000".into(),
                end_time: "00:00:04,000".into(),
                text: "가버렸어요.".into(),
            },
            SubtitleEntry {
                id: 2,
                start_time: "00:00:05,000".into(),
                end_time: "00:00:08,000".into(),
                text: "이건 좀 길어서 중간에 잘린".into(),
            },
            SubtitleEntry {
                id: 3,
                start_time: "00:00:09,000".into(),
                end_time: "00:00:12,000".into(),
                text: "문장이에요 계속?".into(),
            },
        ];

        let span = build_translation_span(&subtitles, 1);
        assert_eq!(span, "이건 좀 길어서 중간에 잘린\n문장이에요 계속?");
    }

    #[test]
    fn build_translation_span_skips_next_when_current_ends_with_question_mark() {
        let subtitles = vec![
            SubtitleEntry {
                id: 1,
                start_time: "00:00:01,000".into(),
                end_time: "00:00:04,000".into(),
                text: "뭐라고?".into(),
            },
            SubtitleEntry {
                id: 2,
                start_time: "00:00:05,000".into(),
                end_time: "00:00:08,000".into(),
                text: "이건 안 붙어야 해요.".into(),
            },
        ];

        let span = build_translation_span(&subtitles, 0);
        assert_eq!(span, "뭐라고?");
    }

    #[test]
    fn build_translation_span_skips_next_when_current_ends_with_exclamation() {
        let subtitles = vec![
            SubtitleEntry {
                id: 1,
                start_time: "00:00:01,000".into(),
                end_time: "00:00:04,000".into(),
                text: "대박!".into(),
            },
            SubtitleEntry {
                id: 2,
                start_time: "00:00:05,000".into(),
                end_time: "00:00:08,000".into(),
                text: "이건 안 붙어야 해요.".into(),
            },
        ];

        let span = build_translation_span(&subtitles, 0);
        assert_eq!(span, "대박!");
    }

    #[test]
    fn build_translation_span_includes_next_when_current_ends_with_comma() {
        let subtitles = vec![
            SubtitleEntry {
                id: 1,
                start_time: "00:00:01,000".into(),
                end_time: "00:00:04,000".into(),
                text: "중간에,".into(),
            },
            SubtitleEntry {
                id: 2,
                start_time: "00:00:05,000".into(),
                end_time: "00:00:08,000".into(),
                text: "계속되는 문장".into(),
            },
        ];

        let span = build_translation_span(&subtitles, 0);
        assert_eq!(span, "중간에,\n계속되는 문장");
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
