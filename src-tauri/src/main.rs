//! Beamer Editor アプリケーションのエントリポイント。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    beamer_editor_app::run();
}
