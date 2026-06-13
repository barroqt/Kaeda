pub mod dictionary;
pub mod embedded_subtitles;
pub mod filter;
pub mod parser;
pub mod session;
pub mod store;
pub mod subtitle;
pub mod tokenizer;
pub mod util;

#[cfg(test)]
pub(crate) fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests")
        .join("fixtures")
        .join(name)
}
