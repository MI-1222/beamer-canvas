//! Git リポジトリ操作のラッパーモジュール。

use crate::error::{GitError, Result};
use git2::{Commit, Delta, DiffOptions, Repository as Git2Repository};
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

/// ファイルの変更種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaStatus {
    /// 変更なし。
    Unmodified,
    /// 新規追加。
    Added,
    /// 削除。
    Deleted,
    /// 変更。
    Modified,
    /// 名前変更。
    Renamed,
    /// コピー。
    Copied,
    /// 無視対象。
    Ignored,
    /// 未追跡。
    Untracked,
    /// ファイル型変更。
    Typechange,
    /// 読み込み不可。
    Unreadable,
    /// コンフリクト。
    Conflicted,
}

impl From<Delta> for DeltaStatus {
    fn from(delta: Delta) -> Self {
        match delta {
            Delta::Unmodified => Self::Unmodified,
            Delta::Added => Self::Added,
            Delta::Deleted => Self::Deleted,
            Delta::Modified => Self::Modified,
            Delta::Renamed => Self::Renamed,
            Delta::Copied => Self::Copied,
            Delta::Ignored => Self::Ignored,
            Delta::Untracked => Self::Untracked,
            Delta::Typechange => Self::Typechange,
            Delta::Unreadable => Self::Unreadable,
            Delta::Conflicted => Self::Conflicted,
        }
    }
}

/// コミット間差分におけるファイル単位の差分情報。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    /// 変更前のファイル相対パス。
    pub old_path: Option<PathBuf>,
    /// 変更後のファイル相対パス。
    pub new_path: Option<PathBuf>,
    /// 変更種別。
    pub status: DeltaStatus,
}

/// Beamer プロジェクト用の Git リポジトリ操作インターフェース。
pub struct BeamerGitRepository {
    inner: Git2Repository,
}

impl BeamerGitRepository {
    /// 指定されたパスから上位階層へ遡って `.git` を自動検出し、リポジトリを開く。
    pub fn discover(path: impl AsRef<Path>) -> Result<Self> {
        let repo =
            Git2Repository::discover(path.as_ref()).map_err(|_e| GitError::RepositoryNotFound {
                path: path.as_ref().display().to_string(),
            })?;
        Ok(Self { inner: repo })
    }

    /// 既存の `git2::Repository` からインスタンスをラップ生成する。
    pub fn new(repo: Git2Repository) -> Self {
        Self { inner: repo }
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
        let bytes = self.read_file_bytes_at_commit(commit_id, file_path)?;
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }

    /// 指定されたコミット ID からファイルの内容バイト列を取得する。
    pub fn read_file_bytes_at_commit(&self, commit_id: &str, file_path: &Path) -> Result<Vec<u8>> {
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

        Ok(blob.content().to_vec())
    }

    /// 2つのコミット間のファイル差分一覧を取得する。
    pub fn diff_commits(&self, old_commit_id: &str, new_commit_id: &str) -> Result<Vec<FileDiff>> {
        let old_oid = git2::Oid::from_str(old_commit_id).map_err(|e| GitError::NotFound {
            message: format!("無効な旧コミットIDです: {}", e),
        })?;
        let new_oid = git2::Oid::from_str(new_commit_id).map_err(|e| GitError::NotFound {
            message: format!("無効な新コミットIDです: {}", e),
        })?;

        let old_commit = self.inner.find_commit(old_oid)?;
        let new_commit = self.inner.find_commit(new_oid)?;

        let old_tree = old_commit.tree()?;
        let new_tree = new_commit.tree()?;

        let diff = self
            .inner
            .diff_tree_to_tree(Some(&old_tree), Some(&new_tree), None)?;

        let mut file_diffs = Vec::new();
        for delta in diff.deltas() {
            let old_path = delta.old_file().path().map(|p| p.to_path_buf());
            let new_path = delta.new_file().path().map(|p| p.to_path_buf());
            let status = DeltaStatus::from(delta.status());

            file_diffs.push(FileDiff {
                old_path,
                new_path,
                status,
            });
        }

        Ok(file_diffs)
    }

    /// 2つのコミット間における Unified Diff パッチ文字列を取得する。
    /// `file_path` を指定した場合は対象ファイルのみの差分を抽出する。
    pub fn diff_patch(
        &self,
        old_commit_id: &str,
        new_commit_id: &str,
        file_path: Option<&Path>,
    ) -> Result<String> {
        let old_oid = git2::Oid::from_str(old_commit_id).map_err(|e| GitError::NotFound {
            message: format!("無効な旧コミットIDです: {}", e),
        })?;
        let new_oid = git2::Oid::from_str(new_commit_id).map_err(|e| GitError::NotFound {
            message: format!("無効な新コミットIDです: {}", e),
        })?;

        let old_commit = self.inner.find_commit(old_oid)?;
        let new_commit = self.inner.find_commit(new_oid)?;

        let old_tree = old_commit.tree()?;
        let new_tree = new_commit.tree()?;

        let mut options = DiffOptions::new();
        if let Some(path) = file_path {
            options.pathspec(path);
        }

        let diff =
            self.inner
                .diff_tree_to_tree(Some(&old_tree), Some(&new_tree), Some(&mut options))?;

        let mut patch_text = String::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            let content = String::from_utf8_lossy(line.content());
            patch_text.push(line.origin());
            patch_text.push_str(&content);
            true
        })?;

        Ok(patch_text)
    }

    /// `git2::Commit` から `CommitSummary` へ変換する。
    fn convert_commit(commit: &Commit<'_>) -> CommitSummary {
        let summary_text = commit.summary().ok().flatten().unwrap_or("").to_string();

        CommitSummary {
            id: commit.id().to_string(),
            summary: summary_text,
            author: commit.author().name().unwrap_or("").to_string(),
            timestamp: commit.time().seconds(),
        }
    }
}
