use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SubtitleExtractError {
    #[error("no text subtitle track found in video")]
    NoSubtitleTracksFound,

    #[error("unsupported text subtitle format: {0}")]
    UnsupportedTextFormat(String),

    #[error("image-based subtitles are not supported")]
    ImageBasedSubtitles,

    #[error("I/O error during subtitle extraction: {0}")]
    IoError(#[from] std::io::Error),

    #[error("internal subtitle extraction error: {0}")]
    InternalError(String),
}

impl SubtitleExtractError {
    pub fn is_retryable_with_ffmpeg(&self) -> bool {
        matches!(
            self,
            Self::UnsupportedTextFormat(_) | Self::InternalError(_)
        )
    }

    pub fn is_final_failure(&self) -> bool {
        matches!(
            self,
            Self::NoSubtitleTracksFound | Self::ImageBasedSubtitles
        )
    }
}

fn duration_to_srt_timestamp(d: Duration) -> String {
    let total_ms = d.as_millis();
    let hours = total_ms / 3_600_000;
    let minutes = (total_ms % 3_600_000) / 60_000;
    let seconds = (total_ms % 60_000) / 1_000;
    let millis = total_ms % 1_000;
    format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}")
}

pub fn extract_to_srt(video_path: &Path) -> Result<PathBuf, SubtitleExtractError> {
    let mut media = unbundle::MediaFile::open(video_path)
        .map_err(|e| SubtitleExtractError::InternalError(e.to_string()))?;

    let mut sub_handle = media.subtitle_track(0).map_err(|e| match e {
        unbundle::UnbundleError::NoSubtitleStream => SubtitleExtractError::NoSubtitleTracksFound,
        other => SubtitleExtractError::InternalError(other.to_string()),
    })?;

    let events = sub_handle
        .extract()
        .map_err(|e| SubtitleExtractError::InternalError(e.to_string()))?;

    let tmp_dir = std::env::temp_dir();
    let stem = video_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("subtitles");
    let tmp_path = tmp_dir.join(format!("kaeda_{stem}.srt"));

    let mut content = String::new();
    for (i, event) in events.iter().enumerate() {
        content.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            duration_to_srt_timestamp(event.start_time),
            duration_to_srt_timestamp(event.end_time),
            event.text,
        ));
    }

    std::fs::write(&tmp_path, content)?;

    Ok(tmp_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_to_srt_timestamp_returns_correct_format() {
        assert_eq!(
            duration_to_srt_timestamp(Duration::from_millis(1000)),
            "00:00:01,000"
        );
        assert_eq!(
            duration_to_srt_timestamp(Duration::from_millis(9500)),
            "00:00:09,500"
        );
        assert_eq!(
            duration_to_srt_timestamp(Duration::from_millis(60_000)),
            "00:01:00,000"
        );
        assert_eq!(
            duration_to_srt_timestamp(Duration::from_millis(3_600_000)),
            "01:00:00,000"
        );
        assert_eq!(
            duration_to_srt_timestamp(Duration::from_millis(3661_234)),
            "01:01:01,234"
        );
    }

    #[test]
    fn extract_to_srt_missing_file_returns_internal_error() {
        let path = Path::new("/nonexistent/video_file.mkv");
        let result = extract_to_srt(path);
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), SubtitleExtractError::InternalError(_)),
            "expected InternalError for missing file"
        );
    }

    #[test]
    fn extract_to_srt_invalid_file_returns_error() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests")
            .join("fixtures")
            .join("empty.srt");
        let result = extract_to_srt(&path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, SubtitleExtractError::InternalError(_)),
            "expected InternalError for non-video file, got: {err}"
        );
    }

    #[test]
    fn is_retryable_returns_true_for_unsupported_text_format() {
        let err = SubtitleExtractError::UnsupportedTextFormat("ASS".into());
        assert!(err.is_retryable_with_ffmpeg());
        assert!(!err.is_final_failure());
    }

    #[test]
    fn is_retryable_returns_true_for_internal_error() {
        let err = SubtitleExtractError::InternalError("something went wrong".into());
        assert!(err.is_retryable_with_ffmpeg());
        assert!(!err.is_final_failure());
    }

    #[test]
    fn is_final_failure_returns_true_for_no_subtitle_tracks() {
        let err = SubtitleExtractError::NoSubtitleTracksFound;
        assert!(!err.is_retryable_with_ffmpeg());
        assert!(err.is_final_failure());
    }

    #[test]
    fn is_final_failure_returns_true_for_image_based_subtitles() {
        let err = SubtitleExtractError::ImageBasedSubtitles;
        assert!(!err.is_retryable_with_ffmpeg());
        assert!(err.is_final_failure());
    }

    #[test]
    fn io_error_is_neither_retryable_nor_final_failure() {
        let err = SubtitleExtractError::IoError(std::io::Error::new(
            std::io::ErrorKind::Other,
            "disk full",
        ));
        assert!(!err.is_retryable_with_ffmpeg());
        assert!(!err.is_final_failure());
    }
}
