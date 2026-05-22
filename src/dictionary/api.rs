use serde::Deserialize;

use crate::dictionary::db::DictEntry;

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
    let trimmed: String = out.chars().filter(|c| !c.is_control()).collect();
    trimmed.trim().to_string()
}

fn normalize_pos(korean: &str) -> &str {
    match korean {
        "명사" => "Noun",
        "동사" => "Verb",
        "형용사" => "Adjective",
        "부사" => "Adverb",
        "대명사" => "Pronoun",
        "감탄사" => "Interjection",
        "조사" => "Particle",
        "관형사" => "Determiner",
        "수사" => "Numeral",
        "접속사" => "Conjunction",
        "의존명사" => "Dependent Noun",
        "보조동사" => "Auxiliary Verb",
        "보조형용사" => "Auxiliary Adjective",
        _ => korean,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NaverResponse {
    #[serde(default)]
    search_result_map: Option<SearchResultMap>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchResultMap {
    #[serde(default)]
    search_result_list_map: Option<SearchResultListMap>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchResultListMap {
    #[serde(default, rename = "WORD")]
    word: Option<WordSection>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WordSection {
    #[serde(default)]
    items: Vec<WordItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WordItem {
    #[serde(default)]
    handle_entry: String,
    #[serde(default)]
    means_collector: Vec<MeansCollector>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MeansCollector {
    #[serde(default)]
    part_of_speech: String,
    #[serde(default)]
    means: Vec<Meaning>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Meaning {
    #[serde(default)]
    value: String,
    #[serde(default)]
    example_ori: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_naver_known_word() {
        let entry = search_naver("사랑").unwrap().expect("should find 사랑");
        assert_eq!(entry.lemma, "사랑");
        assert!(!entry.meaning.is_empty());
        assert_eq!(entry.pos, "Noun");
        assert!(!entry.examples.is_empty());
    }

    #[test]
    fn search_naver_unknown_word() {
        let result = search_naver("zzznonsense123").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn search_naver_verb() {
        let entry = search_naver("먹다").unwrap().expect("should find 먹다");
        assert_eq!(entry.lemma, "먹다");
        assert!(entry.meaning.contains("eat"));
        assert_eq!(entry.pos, "Verb");
    }

    #[test]
    fn search_naver_returns_multiple_meanings() {
        let entry = search_naver("들다").unwrap().expect("should find 들다");
        assert!(!entry.meaning.is_empty());
        assert!(entry.meaning.contains(';'));
    }
}

pub fn search_naver(word: &str) -> Result<Option<DictEntry>, anyhow::Error> {
    let response = ureq::get("https://en.dict.naver.com/api3/koen/search")
        .header("Referer", "https://en.dict.naver.com")
        .query("m", "mobile")
        .query("lang", "ko")
        .query("query", word)
        .call()
        .map_err(|e| anyhow::anyhow!("Naver API request failed: {e}"))?;

    let body = response.into_body().read_to_string()?;
    let parsed: NaverResponse = serde_json::from_str(&body)?;

    let items = parsed
        .search_result_map
        .and_then(|m| m.search_result_list_map)
        .and_then(|l| l.word)
        .map(|w| w.items)
        .unwrap_or_default();

    let item = match items.into_iter().next() {
        Some(item) => item,
        None => return Ok(None),
    };

    let lemma = strip_html_tags(&item.handle_entry);
    let mut meanings: Vec<String> = Vec::new();
    let mut examples: Vec<String> = Vec::new();
    let mut pos = String::new();

    for mc in item.means_collector {
        if pos.is_empty() && !mc.part_of_speech.trim().is_empty() {
            pos = normalize_pos(mc.part_of_speech.trim()).to_string();
        }
        for meaning in mc.means {
            let clean = strip_html_tags(&meaning.value);
            if !clean.is_empty() {
                meanings.push(clean);
            }
            if let Some(ref ex) = meaning.example_ori {
                let clean_ex = strip_html_tags(ex);
                if !clean_ex.is_empty() {
                    examples.push(clean_ex);
                }
            }
        }
    }

    let meaning = if meanings.is_empty() {
        "—".to_string()
    } else {
        meanings.join("; ")
    };

    Ok(Some(DictEntry {
        lemma: if lemma.is_empty() { word.to_string() } else { lemma },
        meaning,
        pos,
        examples,
    }))
}
