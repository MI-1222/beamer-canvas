//! コア機能に関するエラー定義モジュール。

use thiserror::Error;

/// `beamer-core` 内で発生し得るエラーを表す列挙型。
#[derive(Debug, Error)]
pub enum CoreError {
    /// ファイルまたはディレクトリの I/O エラー。
    #[error("I/Oエラーが発生しました: {0}")]
    Io(#[from] std::io::Error),

    /// JSON シリアライズまたはデシリアライズ時のエラー。
    #[error("JSONシリアライズエラーが発生しました: {0}")]
    Json(#[from] serde_json::Error),

    /// 無効なプロジェクト構成またはファイル指定時のエラー。
    #[error("無効なプロジェクト構成です: {message}")]
    InvalidProject {
        /// エラーの詳細メッセージ。
        message: String,
    },

    /// キャッシュ操作時のエラー。
    #[error("キャッシュ操作に失敗しました: {message}")]
    CacheError {
        /// エラーの詳細メッセージ。
        message: String,
    },
}

/// コアモジュールの汎用 Result 型エイリアス。
pub type Result<T> = std::result::Result<T, CoreError>;
