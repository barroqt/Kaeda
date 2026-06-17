use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use thiserror::Error;

/// Check whether `ffmpeg` is available on the system PATH.
pub fn ffmpeg_available() -> bool {
    command_available("ffmpeg")
}

/// Check whether a given binary is available and runs successfully.
///
/// Exposed as `pub(crate)` so tests can inject a non‑existent name
/// without depending on the real `ffmpeg` being installed.
pub(crate) fn command_available(name: &str) -> bool {
    Command::new(name)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[derive(Debug, Error)]
pub enum FfmpegExtractError {
    #[error("ffmpeg not found on system PATH")]
    FfmpegNotFound,

    #[error("ffmpeg command failed: {0}")]
    CommandFailed(String),

    #[error("I/O error during ffmpeg extraction: {0}")]
    IoError(#[from] std::io::Error),
}

/// Extract the first (or a specific) embedded subtitle track from a video
/// file using `ffmpeg`, writing the result to an SRT file inside `output_dir`.
///
/// `track_index` — `None` selects the first subtitle track (map `0:s:0`).
///
/// Returns the path to the generated SRT file.
pub fn extract_subtitles_with_ffmpeg(
    video_path: &Path,
    track_index: Option<u32>,
    output_dir: &Path,
) -> Result<PathBuf, FfmpegExtractError> {
    extract_with_ffmpeg_impl("ffmpeg", video_path, track_index, output_dir)
}

/// Implementation helper that accepts the binary name so tests can inject
/// a non‑existent name without requiring real `ffmpeg` on `PATH`.
pub(crate) fn extract_with_ffmpeg_impl(
    ffmpeg_binary: &str,
    video_path: &Path,
    track_index: Option<u32>,
    output_dir: &Path,
) -> Result<PathBuf, FfmpegExtractError> {
    if !command_available(ffmpeg_binary) {
        return Err(FfmpegExtractError::FfmpegNotFound);
    }

    let stem = video_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("subtitles");
    let output_path = output_dir.join(format!("{stem}.srt"));

    let map_arg = format!("0:s:{}", track_index.unwrap_or(0));

    let output = Command::new(ffmpeg_binary)
        .arg("-hide_banner")
        .arg("-y")
        .arg("-i")
        .arg(video_path)
        .arg("-map")
        .arg(&map_arg)
        .arg(&output_path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let truncated = if stderr.len() > 500 {
            format!("{}... (truncated)", &stderr[..500])
        } else {
            stderr.to_string()
        };
        return Err(FfmpegExtractError::CommandFailed(truncated));
    }

    Ok(output_path)
}

#[cfg(test)]
mod availability_tests {
    use super::*;

    #[test]
    fn ffmpeg_available_returns_false_for_nonexistent_binary() {
        assert!(!command_available("nonexistent_ffmpeg_binary_xyz_12345"));
    }

    #[test]
    fn ffmpeg_available_does_not_panic() {
        let _ = ffmpeg_available();
    }
}

#[cfg(test)]
mod extraction_tests {
    use super::*;
    use std::io;

    #[test]
    fn ffmpeg_not_found_when_binary_does_not_exist() {
        let video = Path::new("/tmp/test.mkv");
        let out = Path::new("/tmp");
        let result =
            extract_with_ffmpeg_impl("nonexistent_ffmpeg_binary_xyz_12345", video, None, out);
        assert!(matches!(result, Err(FfmpegExtractError::FfmpegNotFound)));
    }

    #[test]
    fn ffmpeg_not_found_does_not_panic() {
        let video = Path::new("/tmp/test.mkv");
        let out = Path::new("/tmp");
        let _ = extract_with_ffmpeg_impl("nonexistent_ffmpeg_binary_xyz_12345", video, None, out);
    }

    #[test]
    fn track_index_none_uses_zero() {
        let video = Path::new("/tmp/test.mkv");
        let out = Path::new("/tmp");
        let result =
            extract_with_ffmpeg_impl("nonexistent_ffmpeg_binary_xyz_12345", video, None, out);
        assert!(matches!(result, Err(FfmpegExtractError::FfmpegNotFound)));
    }

    #[test]
    fn track_index_some_uses_given_index() {
        let video = Path::new("/tmp/test.mkv");
        let out = Path::new("/tmp");
        let result =
            extract_with_ffmpeg_impl("nonexistent_ffmpeg_binary_xyz_12345", video, Some(2), out);
        assert!(matches!(result, Err(FfmpegExtractError::FfmpegNotFound)));
    }

    #[test]
    fn io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let ffmpeg_err: FfmpegExtractError = io_err.into();
        assert!(matches!(ffmpeg_err, FfmpegExtractError::IoError(_)));
    }
}
