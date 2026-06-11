pub use crate::subtitle::{CoreError, SubtitleEntry, entries_from_srt};

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
