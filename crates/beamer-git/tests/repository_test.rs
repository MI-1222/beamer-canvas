//! `git2` リポジトリ探索およびコミット差分取得の統合テスト。

use beamer_git::{BeamerGitRepository, DeltaStatus, GitError};
use git2::{Commit, Repository as Git2Repository, Signature};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// テスト用の Git リポジトリを作成し、`BeamerGitRepository` と初期化ディレクトリを返す。
struct TestRepoContext {
    /// 一時ディレクトリ（スコープ終了時に自動クリーンアップ）。
    _temp_dir: TempDir,
    /// ラップされたリポジトリ。
    pub beamer_repo: BeamerGitRepository,
    /// 内部の生 git2 リポジトリ。
    pub raw_repo: Git2Repository,
}

impl TestRepoContext {
    fn new() -> Self {
        let temp_dir = TempDir::new().expect("一時ディレクトリの作成失敗。");
        let raw_repo = Git2Repository::init(temp_dir.path()).expect("Git リポジトリ初期化失敗。");

        // コミット作成に必要な基本設定を行う。
        let mut config = raw_repo.config().expect("リポジトリ設定取得失敗。");
        config
            .set_str("user.name", "Test User")
            .expect("ユーザー名設定失敗。");
        config
            .set_str("user.email", "test@example.com")
            .expect("メールアドレス設定失敗。");

        let beamer_repo = BeamerGitRepository::discover(temp_dir.path())
            .expect("作成したリポジトリの discover に失敗しました。");

        Self {
            _temp_dir: temp_dir,
            beamer_repo,
            raw_repo,
        }
    }

    /// ファイルを書き込んでインデックスに追加し、コミットを作成する。
    fn create_commit(&self, message: &str, files: &[(&str, &str)], parents: &[&Commit]) -> String {
        let workdir = self.raw_repo.workdir().expect("workdir が存在すること。");
        let mut index = self.raw_repo.index().expect("Index 取得失敗。");

        for (rel_path, content) in files {
            let full_path = workdir.join(rel_path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent).expect("親ディレクトリ作成失敗。");
            }
            fs::write(&full_path, content).expect("ファイル書き込み失敗。");
            index
                .add_path(Path::new(rel_path))
                .expect("インデックスへの追加失敗。");
        }

        index.write().expect("インデックス書き込み失敗。");
        let tree_id = index.write_tree().expect("Tree 生成失敗。");
        let tree = self.raw_repo.find_tree(tree_id).expect("Tree 取得失敗。");
        let signature = Signature::now("Test User", "test@example.com").expect("署名作成失敗。");

        let oid = self
            .raw_repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                parents,
            )
            .expect("コミット生成失敗。");

        oid.to_string()
    }

    /// ファイルを削除してコミットを作成する。
    fn create_delete_commit(
        &self,
        message: &str,
        delete_files: &[&str],
        parents: &[&Commit],
    ) -> String {
        let workdir = self.raw_repo.workdir().expect("workdir が存在すること。");
        let mut index = self.raw_repo.index().expect("Index 取得失敗。");

        for rel_path in delete_files {
            let full_path = workdir.join(rel_path);
            if full_path.exists() {
                fs::remove_file(&full_path).expect("ファイル削除失敗。");
            }
            index
                .remove_path(Path::new(rel_path))
                .expect("インデックスからの削除失敗。");
        }

        index.write().expect("インデックス書き込み失敗。");
        let tree_id = index.write_tree().expect("Tree 生成失敗。");
        let tree = self.raw_repo.find_tree(tree_id).expect("Tree 取得失敗。");
        let signature = Signature::now("Test User", "test@example.com").expect("署名作成失敗。");

        let oid = self
            .raw_repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                parents,
            )
            .expect("コミット生成失敗。");

        oid.to_string()
    }
}

#[test]
fn test_discover_from_nested_subdirectory() {
    let ctx = TestRepoContext::new();
    let workdir = ctx.raw_repo.workdir().expect("workdir 取得失敗。");

    // 深い階層のサブディレクトリを作成する。
    let nested_dir = workdir.join("subproject").join("slides").join("assets");
    fs::create_dir_all(&nested_dir).expect("ネストディレクトリの作成失敗。");

    // ネストした子ディレクトリから上位の Git リポジトリを検出できることを確認する。
    let discovered = BeamerGitRepository::discover(&nested_dir);
    assert!(
        discovered.is_ok(),
        "子ディレクトリからの探索に成功すること。"
    );

    let discovered_repo = discovered.unwrap();
    let discovered_workdir = discovered_repo.workdir().expect("workdir が存在すること。");

    // 正確に同一のルートパスに到達していることを検証する。
    let canonical_workdir = workdir.canonicalize().expect("パス正規化失敗。");
    let canonical_discovered = discovered_workdir.canonicalize().expect("パス正規化失敗。");
    assert_eq!(canonical_discovered, canonical_workdir);
}

#[test]
fn test_discover_non_existent_or_non_git_directory() {
    let empty_temp = TempDir::new().expect("一時ディレクトリ作成失敗。");
    let result = BeamerGitRepository::discover(empty_temp.path());

    match result {
        Err(GitError::RepositoryNotFound { path }) => {
            assert!(path.contains(empty_temp.path().to_str().unwrap()));
        }
        _ => panic!("GitError::RepositoryNotFound が返されること。"),
    }
}

#[test]
fn test_diff_commits_and_file_content_extraction() {
    let ctx = TestRepoContext::new();

    // 1. 初回コミットを作成する。
    let initial_slides = r#"\documentclass{beamer}
\begin{document}
\begin{frame}{Version 1}
  First content.
\end{frame}
\end{document}"#;

    let commit1_id = ctx.create_commit("Initial slides", &[("main.tex", initial_slides)], &[]);

    // 2. 2回目のコミット（main.tex 変更 + appendix.tex 新規追加）を作成する。
    let commit1 = ctx
        .raw_repo
        .find_commit(git2::Oid::from_str(&commit1_id).unwrap())
        .unwrap();
    let updated_slides = r#"\documentclass{beamer}
\begin{document}
\begin{frame}{Version 2}
  Updated content for visual diff.
\end{frame}
\end{document}"#;
    let appendix_content = r#"\section{Appendix}
\begin{frame}{Extra}
  Supplemental note.
\end{frame}"#;

    let commit2_id = ctx.create_commit(
        "Update main.tex and add appendix.tex",
        &[
            ("main.tex", updated_slides),
            ("appendix.tex", appendix_content),
        ],
        &[&commit1],
    );

    // 3. 3回目のコミット（appendix.tex 削除）を作成する。
    let commit2 = ctx
        .raw_repo
        .find_commit(git2::Oid::from_str(&commit2_id).unwrap())
        .unwrap();
    let commit3_id =
        ctx.create_delete_commit("Remove appendix.tex", &["appendix.tex"], &[&commit2]);

    // --- ツリー差分の検証 ---

    // commit1 -> commit2 の差分を取得する。
    let diff_1_to_2 = ctx
        .beamer_repo
        .diff_commits(&commit1_id, &commit2_id)
        .expect("commit1 から commit2 への差分取得に失敗しました。");

    assert_eq!(diff_1_to_2.len(), 2, "2件のファイル差分が検出されること。");

    let main_diff = diff_1_to_2
        .iter()
        .find(|d| d.new_path.as_deref() == Some(Path::new("main.tex")))
        .expect("main.tex の差分が存在すること。");
    assert_eq!(
        main_diff.status,
        DeltaStatus::Modified,
        "main.tex は Modified であること。"
    );

    let appendix_diff = diff_1_to_2
        .iter()
        .find(|d| d.new_path.as_deref() == Some(Path::new("appendix.tex")))
        .expect("appendix.tex の差分が存在すること。");
    assert_eq!(
        appendix_diff.status,
        DeltaStatus::Added,
        "appendix.tex は Added であること。"
    );

    // commit2 -> commit3 の差分を取得する。
    let diff_2_to_3 = ctx
        .beamer_repo
        .diff_commits(&commit2_id, &commit3_id)
        .expect("commit2 から commit3 への差分取得に失敗しました。");

    assert_eq!(
        diff_2_to_3.len(),
        1,
        "1件のファイル削除差分が検出されること。"
    );
    let deleted_diff = &diff_2_to_3[0];
    assert_eq!(
        deleted_diff.old_path.as_deref(),
        Some(Path::new("appendix.tex"))
    );
    assert_eq!(
        deleted_diff.status,
        DeltaStatus::Deleted,
        "appendix.tex は Deleted であること。"
    );

    // --- ファイル内容抽出の検証 ---

    // commit1 時点の main.tex を取得する。
    let content_at_c1 = ctx
        .beamer_repo
        .read_file_at_commit(&commit1_id, Path::new("main.tex"))
        .expect("commit1 の main.tex 抽出失敗。");
    assert_eq!(
        content_at_c1, initial_slides,
        "commit1 時点の内容が正確に取得できること。"
    );

    // commit2 時点の main.tex を取得する。
    let content_at_c2 = ctx
        .beamer_repo
        .read_file_at_commit(&commit2_id, Path::new("main.tex"))
        .expect("commit2 の main.tex 抽出失敗。");
    assert_eq!(
        content_at_c2, updated_slides,
        "commit2 時点の内容が正確に取得できること。"
    );

    // commit3 時点では appendix.tex が存在しないためエラーになることを検証する。
    let read_deleted = ctx
        .beamer_repo
        .read_file_at_commit(&commit3_id, Path::new("appendix.tex"));
    assert!(
        read_deleted.is_err(),
        "削除済みコミットからの取得はエラーになること。"
    );

    // --- Unified Diff パッチの検証 ---
    let patch = ctx
        .beamer_repo
        .diff_patch(&commit1_id, &commit2_id, Some(Path::new("main.tex")))
        .expect("main.tex の差分パッチ取得失敗。");

    assert!(
        patch.contains("-  First content."),
        "削除行がパッチに含まれること。"
    );
    assert!(
        patch.contains("+  Updated content for visual diff."),
        "追加行がパッチに含まれること。"
    );
}
