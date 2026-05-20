use std::fs;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use rusqlite::Connection;

use lantern::app;
use lantern::dictionary;
use lantern::filter::{FilterConfig, load_frequency_list, load_known_list};
use lantern::parser::srt::parse_srt;
use lantern::store::{init_store, get_stats, add_known_word, list_known_words};

#[derive(Parser)]
#[command(name = "lantern", about = "Korean vocabulary mining TUI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a mining session on an SRT file
    Mine {
        /// Path to SRT subtitle file
        file: PathBuf,
    },
    /// Show deck and session statistics
    Stats,
    /// Manage known words
    Known {
        #[command(subcommand)]
        command: KnownCommands,
    },
}

#[derive(Subcommand)]
enum KnownCommands {
    /// Add a word to the known list
    Add {
        /// Lemma to mark as known
        word: String,
    },
    /// List all known words
    List,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Mine { file } => cmd_mine(file),
        Commands::Stats => cmd_stats(),
        Commands::Known { command } => match command {
            KnownCommands::Add { word } => cmd_known_add(word),
            KnownCommands::List => cmd_known_list(),
        },
    }
}

fn data_dir() -> PathBuf {
    PathBuf::from("./.srt-miner")
}

fn db_path() -> PathBuf {
    data_dir().join("srt-miner.db")
}

fn open_db() -> anyhow::Result<Connection> {
    fs::create_dir_all(data_dir())?;
    let conn = Connection::open(db_path())?;
    init_store(&conn)?;
    Ok(conn)
}

fn cmd_mine(file: PathBuf) -> anyhow::Result<()> {
    let conn = open_db()?;

    let needs_build = conn
        .query_row("SELECT COUNT(*) FROM dictionary", [], |row| {
            let count: i64 = row.get(0)?;
            Ok(count == 0)
        })
        .unwrap_or(true);

    if needs_build {
        let dict_tsv = data_dir().join("dictionary.tsv");
        if dict_tsv.exists() {
            eprintln!("Building dictionary index…");
            dictionary::db::build_index(&conn, dict_tsv.to_string_lossy().as_ref())?;
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM dictionary", [], |row| row.get(0))
                .unwrap_or(0);
            eprintln!("Dictionary ready ({count} entries)");
        }
    }

    let frequency_set =
        load_frequency_list(&data_dir().join("frequency.txt").to_string_lossy())?;
    let known_set = load_known_list(&data_dir().join("known.txt").to_string_lossy())?;
    let config = FilterConfig {
        frequency_set,
        known_set,
    };

    let tokenizer = lantern::tokenizer::korean::KoreanTokenizer::new()?;
    let subtitles = parse_srt(&file.to_string_lossy())?;
    let mut state = app::AppState::new(subtitles, file.to_string_lossy().to_string(), &tokenizer);

    app::run(&mut state, &conn, &config)
}

fn cmd_stats() -> anyhow::Result<()> {
    let conn = open_db()?;
    let stats = get_stats(&conn)?;

    println!("{:<20} {:>8}", "Metric", "Value");
    println!("{:<20} {:>8}", "──────", "─────");
    println!("{:<20} {:>8}", "total words", stats.total_words);
    println!("{:<20} {:>8}", "added today", stats.added_today);
    println!("{:<20} {:>8}", "known words", stats.total_known);

    Ok(())
}

fn cmd_known_add(word: String) -> anyhow::Result<()> {
    let conn = open_db()?;
    let known_path = data_dir().join("known.txt");
    add_known_word(&conn, &word, &known_path.to_string_lossy())?;
    println!("marked '{word}' as known");
    Ok(())
}

fn cmd_known_list() -> anyhow::Result<()> {
    let conn = open_db()?;
    let words = list_known_words(&conn)?;
    for lemma in &words {
        println!("{lemma}");
    }
    Ok(())
}
