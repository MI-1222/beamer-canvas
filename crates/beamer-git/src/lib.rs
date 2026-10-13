//! `git2` を用いた Beamer プロジェクトのバージョン管理および履歴探索を提供するクレート。
//!
//! リポジトリの自動探索、過去コミットからのスライドソース抽出、コミット要約の取得を担当する。

pub mod error;
pub mod repository;

pub use error::{GitError, Result};
pub use repository::{BeamerGitRepository, CommitSummary, DeltaStatus, FileDiff};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_current_repo() {
        let repo = BeamerGitRepository::discover(".")
            .expect("カレントディレクトリのGitリポジトリ検出に失敗しました。");
        assert!(repo.workdir().is_some());
    }
}
