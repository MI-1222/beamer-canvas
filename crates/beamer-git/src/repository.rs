//! Git リポジトリ操作のラッパーモジュール。

use crate::error::{GitError, Result};
use git2::{Commit, Repository as Git2Repository};
use std::path::{Path, PathBuf};

/// コミットの要約メタデータを保持する構造体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSummary {
    /// コミット ID（SHA-1 ハッシュ文字列）。
    pub id: String,
    /// コミットメッセージの先頭行。
    pub summary: String,
    /// コミッター名。
    pub author: String,
    /// コミット日時（Unix タイムスタンプ秒）。
    pub timestamp: i64,
}

/// Beamer プロジェクト用の Git リポジトリ操作インターフェース。
pub struct BeamerGitRepository {
    inner: Git2Repository,
}

impl BeamerGitRepository {
    /// 指定されたパスから上位階層へ遡って `.git` を自動検出し、リポジトリを開く。
    pub fn discover(path: impl AsRef<Path>) -> Result<Self> {
        let repo = Git2Repository::discover(path.as_ref()).map_err(|_e| {
            GitError::RepositoryNotFound {
                path: path.as_ref().display().to_string(),
            }
        })?;
        Ok(Self { inner: repo })
    }

    /// リポジトリの作業ディレクトリのパスを取得する。
    pub fn workdir(&self) -> Option<PathBuf> {
        self.inner.workdir().map(|p| p.to_path_buf())
    }

    /// 現在の HEAD コミットを取得する。
    pub fn head_commit(&self) -> Result<CommitSummary> {
        let head = self.inner.head()?;
        let commit = head.peel_to_commit()?;
        Ok(Self::convert_commit(&commit))
    }

    /// 指定されたコミット ID からファイルの内容文字列を取得する。
    pub fn read_file_at_commit(&self, commit_id: &str, file_path: &Path) -> Result<String> {
        let oid = git2::Oid::from_str(commit_id).map_err(|e| GitError::NotFound {
            message: format!("無効なコミットIDです: {}", e),
        })?;
        let commit = self.inner.find_commit(oid)?;
        let tree = commit.tree()?;
        let entry = tree.get_path(file_path).map_err(|e| GitError::NotFound {
            message: format!("ファイルが指定コミット内に見つかりません: {}", e),
        })?;
        let object = entry.to_object(&self.inner)?;
        let blob = object.as_blob().ok_or_else(|| GitError::NotFound {
            message: "対象オブジェクトがファイル（Blob）ではありません。".to_string(),
        })?;

        let content = String::from_utf8_lossy(blob.content()).to_string();
        Ok(content)
    }

    /// `git2::Commit` から `CommitSummary` へ変換する。
    fn convert_commit(commit: &Commit<'_>) -> CommitSummary {
        let summary_text = commit
            .summary()
            .ok()
            .flatten()
            .unwrap_or("")
            .to_string();

        CommitSummary {
            id: commit.id().to_string(),
            summary: summary_text,
            author: commit.author().name().unwrap_or("").to_string(),
            timestamp: commit.time().seconds(),
        }
    }
}
