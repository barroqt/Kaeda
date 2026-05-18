#[derive(Debug, Clone)]
pub struct Subtitle {
    pub index: u32,
    pub timestamp: String,
    pub text: String,
}

fn strip_html_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

pub fn parse_srt(path: &str) -> anyhow::Result<Vec<Subtitle>> {
    let content = std::fs::read_to_string(path)?;
    let mut subtitles = Vec::new();

    for block in content.split("\n\n") {
        let block = block.trim();

        if block.is_empty() {
            continue;
        }

        let mut lines = block.lines();

        let index = lines
            .next()
            .and_then(|l| l.parse::<u32>().ok())
            .ok_or_else(|| anyhow::anyhow!("missing sequence number"))?;
        let timestamp = lines
            .next()
            .ok_or_else(|| anyhow::anyhow!("missing timestamp"))?
            .to_string();
        let text = lines.collect::<Vec<_>>().join("\n");
        let text = strip_html_tags(&text);

        subtitles.push(Subtitle {
            index,
            timestamp,
            text,
        });
    }

    Ok(subtitles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_returns_correct_count() {
        let subtitles = parse_srt("tests/fixtures/sample.srt").unwrap();
        assert_eq!(subtitles.len(), 5);
    }

    #[test]
    fn parse_preserves_timestamp() {
        let subtitles = parse_srt("tests/fixtures/sample.srt").unwrap();
        assert_eq!(subtitles[0].timestamp, "00:00:01,000 --> 00:00:04,000");
    }

    #[test]
    fn parse_strips_html_tags() {
        let subtitles = parse_srt("tests/fixtures/sample.srt").unwrap();
        assert_eq!(subtitles[0].text, "안녕하세요 반갑습니다.");
    }
}
