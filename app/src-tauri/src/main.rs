// minimal Tauri bootstrap — verifies core dependency is wired
use kaeda_core::parser::srt::Subtitle;

fn main() {
    // ensure the core crate compiles and links
    let _subtitle = Subtitle {
        index: 0,
        timestamp: String::new(),
        text: String::new(),
    };

    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
