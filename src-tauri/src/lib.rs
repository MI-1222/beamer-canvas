//! Tauri v2 デスクトップアプリケーション本体ロジック。

/// アプリケーション起動時に Tauri ランタイムを初期化して実行する。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|_app| {
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Tauriアプリケーション実行中にエラーが発生しました。");
}
