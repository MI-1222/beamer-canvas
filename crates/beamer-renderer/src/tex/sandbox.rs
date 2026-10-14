//! `TeX` ビルド作業用の一時ディレクトリおよびサンドボックス環境を管理するモジュール。
//!
//! プロジェクトルートを中間ファイルで汚染させないための隔離、
//! フルコンパイルの中間ファイル(`.aux`, `.nav` 等)の保護、
//! 部分コンパイル時へのコピー、および `TEXINPUTS` 環境変数の構築を担当する。

use crate::error::{RenderError, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// ビルドの種別を表す列挙型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildKind {
    /// 全スライドを含む完全なフルコンパイル。
    Full,
    /// 特定のスライドフレームのみを対象とする部分コンパイル。
    Partial {
        /// 対象フレームのラベル(未指定時はインメモリ仮想ラベルを使用)。
        frame_label: Option<String>,
    },
}

/// サンドボックス環境の設定を保持する構造体。
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// プロジェクトのルートディレクトリ。
    pub project_root: PathBuf,
    /// ビルドディレクトリのベース相対パス(デフォルト: `.build`)。
    pub build_dir_name: PathBuf,
}

impl SandboxConfig {
    /// デフォルト設定でインスタンスを生成する。
    pub fn new(project_root: impl AsRef<Path>) -> Self {
        Self {
            project_root: project_root.as_ref().to_path_buf(),
            build_dir_name: PathBuf::from(".build"),
        }
    }
}

/// サンドボックス環境の実行状態とファイルパスを管理する構造体。
#[derive(Debug)]
pub struct Sandbox {
    /// 設定情報。
    config: SandboxConfig,
    /// フルコンパイル用の作業ディレクトリ。
    full_build_dir: PathBuf,
    /// 部分コンパイル用の作業ディレクトリ。
    partial_build_dir: PathBuf,
}

impl Sandbox {
    /// 指定された設定に基づいてサンドボックス環境を初期化する。
    pub fn new(config: SandboxConfig) -> Result<Self> {
        let base = config.project_root.join(&config.build_dir_name);
        let full_build_dir = base.join("full");
        let partial_build_dir = base.join("partial");

        fs::create_dir_all(&full_build_dir).map_err(|e| RenderError::Sandbox {
            message: format!("フルコンパイルディレクトリの作成に失敗しました: {e}"),
        })?;

        fs::create_dir_all(&partial_build_dir).map_err(|e| RenderError::Sandbox {
            message: format!("部分コンパイルディレクトリの作成に失敗しました: {e}"),
        })?;

        Ok(Self {
            config,
            full_build_dir,
            partial_build_dir,
        })
    }

    /// ビルド種別に応じた作業出力ディレクトリを取得する。
    pub fn output_dir(&self, kind: &BuildKind) -> &Path {
        match kind {
            BuildKind::Full => &self.full_build_dir,
            BuildKind::Partial { .. } => &self.partial_build_dir,
        }
    }

    /// フルコンパイル用の作業ディレクトリを取得する。
    pub fn full_build_dir(&self) -> &Path {
        &self.full_build_dir
    }

    /// 部分コンパイル用の作業ディレクトリを取得する。
    pub fn partial_build_dir(&self) -> &Path {
        &self.partial_build_dir
    }

    /// プロジェクトルートのパスを取得する。
    pub fn project_root(&self) -> &Path {
        &self.config.project_root
    }

    /// 部分コンパイルの実行前に、フルコンパイルのマスター中間ファイルを部分用ディレクトリへ複製する。
    ///
    /// これにより、部分コンパイル時にも総ページ数カウンタや目次、
    /// 相互参照情報を維持したまま高速に組版できる。
    pub fn prepare_partial_sandbox(&self, stem: &str) -> Result<()> {
        let extensions = ["aux", "nav", "toc", "out", "snm"];

        for ext in extensions {
            let src = self.full_build_dir.join(format!("{stem}.{ext}"));
            let dst = self.partial_build_dir.join(format!("{stem}.{ext}"));

            if src.exists() {
                fs::copy(&src, &dst).map_err(|e| RenderError::Sandbox {
                    message: format!(
                        "マスター中間ファイル {ext} のコピーに失敗しました ({} -> {}): {e}",
                        src.display(),
                        dst.display()
                    ),
                })?;
            }
        }

        Ok(())
    }

    /// フルコンパイルまたは部分コンパイル完了後の成果物pdfパスを取得する。
    pub fn get_pdf_path(&self, kind: &BuildKind, stem: &str) -> PathBuf {
        self.output_dir(kind).join(format!("{stem}.pdf"))
    }

    /// フルコンパイル出力の `.nav` ファイルのパスを取得する。
    pub fn get_master_nav_path(&self, stem: &str) -> PathBuf {
        self.full_build_dir.join(format!("{stem}.nav"))
    }

    /// 作業ディレクトリに応じた `TEXINPUTS` 環境変数文字列を生成する。
    ///
    /// プロジェクトルート配下の画像や外部ファイルを検索可能にしつつ、
    /// システムの `TeX` 配布物パスを末尾のデリミタで維持する。
    pub fn build_texinputs(&self, current_workdir: &Path) -> String {
        let separator = if cfg!(windows) { ";" } else { ":" };

        // 探索優先順位:
        // 1. カレント作業ディレクトリ。
        // 2. プロジェクトルートとその再帰探索 (//)。
        // 3. 既存の `TEXINPUTS` 環境変数(存在する場合)。
        // 4. システム標準パス(末尾のセパレータ)。
        let mut parts = Vec::new();

        parts.push(".".to_string());
        parts.push(current_workdir.to_string_lossy().to_string());

        let project_root_str = self.config.project_root.to_string_lossy();
        if cfg!(windows) {
            parts.push(format!("{project_root_str}\\"));
        } else {
            parts.push(format!("{project_root_str}//"));
        }

        if let Some(existing) = env::var("TEXINPUTS").ok().filter(|s| !s.trim().is_empty()) {
            parts.push(existing);
        }

        // 末尾にセパレータを付与してシステムの `TeX` 検索パスへフォールバックさせる。
        format!("{}{}", parts.join(separator), separator)
    }

    /// サンドボックス内の一時生成ファイルを安全にクリーンアップする。
    pub fn clean_all(&self) -> Result<()> {
        if self.full_build_dir.exists() {
            fs::remove_dir_all(&self.full_build_dir).map_err(|e| RenderError::Sandbox {
                message: format!("フルビルドディレクトリの削除に失敗しました: {e}"),
            })?;
        }
        if self.partial_build_dir.exists() {
            fs::remove_dir_all(&self.partial_build_dir).map_err(|e| RenderError::Sandbox {
                message: format!("部分ビルドディレクトリの削除に失敗しました: {e}"),
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sandbox_directories_creation() {
        let temp = TempDir::new().expect("一時ディレクトリの作成に失敗。");
        let config = SandboxConfig::new(temp.path());
        let sandbox = Sandbox::new(config).expect("サンドボックスの初期化に失敗。");

        assert!(sandbox.full_build_dir().exists());
        assert!(sandbox.partial_build_dir().exists());
        assert_eq!(
            sandbox.output_dir(&BuildKind::Full),
            sandbox.full_build_dir()
        );
        assert_eq!(
            sandbox.output_dir(&BuildKind::Partial { frame_label: None }),
            sandbox.partial_build_dir()
        );
    }

    #[test]
    fn test_master_aux_and_nav_copy_to_partial() {
        let temp = TempDir::new().expect("一時ディレクトリの作成に失敗。");
        let config = SandboxConfig::new(temp.path());
        let sandbox = Sandbox::new(config).expect("サンドボックスの初期化に失敗。");

        // フルコンパイル側に擬似的なマスター `.aux` と `.nav` を作成する。
        let aux_file = sandbox.full_build_dir().join("main.aux");
        let nav_file = sandbox.full_build_dir().join("main.nav");
        fs::write(&aux_file, "\\relax\n\\gdef\\totalpages{5}").unwrap();
        fs::write(&nav_file, "\\headcommand{\\slideentry{1}{0}{1}{1/5}{}{0}}").unwrap();

        sandbox
            .prepare_partial_sandbox("main")
            .expect("中間ファイルの複製に失敗。");

        let partial_aux = sandbox.partial_build_dir().join("main.aux");
        let partial_nav = sandbox.partial_build_dir().join("main.nav");

        assert!(partial_aux.exists());
        assert!(partial_nav.exists());
        assert_eq!(
            fs::read_to_string(&partial_aux).unwrap(),
            "\\relax\n\\gdef\\totalpages{5}"
        );
        assert_eq!(
            fs::read_to_string(&partial_nav).unwrap(),
            "\\headcommand{\\slideentry{1}{0}{1}{1/5}{}{0}}"
        );
    }

    #[test]
    fn test_build_texinputs() {
        let temp = TempDir::new().expect("一時ディレクトリの作成に失敗。");
        let config = SandboxConfig::new(temp.path());
        let sandbox = Sandbox::new(config).expect("サンドボックスの初期化に失敗。");

        let texinputs = sandbox.build_texinputs(sandbox.full_build_dir());
        let separator = if cfg!(windows) { ";" } else { ":" };

        assert!(texinputs.ends_with(separator));
        assert!(texinputs.contains(&temp.path().to_string_lossy().to_string()));
    }
}
