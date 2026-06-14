use std::process::{Command, Stdio};

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

#[cfg(test)]
mod tests {
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
