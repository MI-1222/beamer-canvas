//! Git 操作に関するエラー定義モジュール。

use thiserror::Error;

/// Git リポジトリ操作中に発生し得るエラー型。
#[derive(Debug, Error)]
pub enum GitError {
    /// `git2` ライブラリ内部のエラー。
    #[error("Git操作エラー: {0}")]
    Git(#[from] git2::Error),

    /// 指定されたパスに Git リポジトリが存在しない場合のエラー。
    #[error("Gitリポジトリが見つかりません: {path}")]
    RepositoryNotFound {
        /// 対象の検索開始パス。
        path: String,
    },

    /// コミットやオブジェクトの探索失敗。
    #[error("Gitオブジェクトの取得に失敗しました: {message}")]
    NotFound {
        /// エラーの詳細メッセージ。
        message: String,
    },
}

/// Git モジュールの汎用 Result 型エイリアス。
pub type Result<T> = std::result::Result<T, GitError>;
