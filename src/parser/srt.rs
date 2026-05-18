#[derive(Debug, Clone)]
pub struct Subtitle {
    pub index: u32,
    pub timestamp: String,
    pub text: String,
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
}
