//! デスクトップアプリケーションの本体ロジック。

pub mod commands;

pub use commands::{BitmapPayload, generate_test_bitmap};

/// アプリケーション起動時にTauriランタイムを初期化して実行する。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::generate_test_bitmap])
        .setup(|_app| Ok(()))
        .run(tauri::generate_context!())
        .expect("Tauriアプリケーション実行中にエラーが発生しました。");
}
