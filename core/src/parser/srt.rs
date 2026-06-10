#[derive(Debug, Clone)]
pub struct Subtitle {
    pub index: u32,
    pub timestamp: String,
    pub text: String,
}

use crate::util::strip_html_tags;

pub fn parse_srt(path: &str) -> anyhow::Result<Vec<Subtitle>> {
    let content = std::fs::read_to_string(path)?;
    let mut subtitles = Vec::new();

    for block in content.split("\n\n") {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }

        let mut lines = block.lines();

        let Some(index) = lines.next().and_then(|l| l.parse::<u32>().ok()) else {
            continue;
        };
        let Some(timestamp) = lines.next() else {
            continue;
        };
        let text = strip_html_tags(&lines.collect::<Vec<_>>().join("\n"));
        if text.trim().is_empty() {
            continue;
        }

        subtitles.push(Subtitle {
            index,
            timestamp: timestamp.to_string(),
            text,
        });
    }

    Ok(subtitles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture_path;

    #[test]
    fn parse_returns_correct_count() {
        let path = fixture_path("sample.srt");
        let subtitles = parse_srt(&path.to_string_lossy()).unwrap();
        assert_eq!(subtitles.len(), 5);
    }

    #[test]
    fn parse_preserves_timestamp() {
        let path = fixture_path("sample.srt");
        let subtitles = parse_srt(&path.to_string_lossy()).unwrap();
        assert_eq!(subtitles[0].timestamp, "00:00:01,000 --> 00:00:04,000");
    }

    #[test]
    fn parse_strips_html_tags() {
        let path = fixture_path("sample.srt");
        let subtitles = parse_srt(&path.to_string_lossy()).unwrap();
        assert_eq!(subtitles[0].text, "안녕하세요 반갑습니다.");
    }

    #[test]
    fn parse_skips_malformed_blocks() {
        let path = fixture_path("sample_malformed.srt");
        let subtitles = parse_srt(&path.to_string_lossy()).unwrap();
        assert_eq!(subtitles.len(), 2);
        assert_eq!(subtitles[0].index, 1);
        assert_eq!(subtitles[1].index, 4);
    }

    #[test]
    fn parse_empty_file_returns_empty() {
        let path = fixture_path("empty.srt");
        let subtitles = parse_srt(&path.to_string_lossy()).unwrap();
        assert!(subtitles.is_empty());
    }
}
