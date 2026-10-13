//! プロジェクト管理およびメタデータ定義モジュール。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Beamer プロジェクトの設定情報を保持する構造体。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectConfig {
    /// メインとなる LaTeX ファイルの相対パス。
    pub main_file: PathBuf,
    /// TeX コンパイルコマンド（例: `latexmk`）。
    pub build_command: String,
    /// コンパイルオプション引数のリスト。
    pub build_args: Vec<String>,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            main_file: PathBuf::from("main.tex"),
            build_command: "latexmk".to_string(),
            build_args: vec![
                "-lualatex".to_string(),
                "-interaction=nonstopmode".to_string(),
                "-synctex=1".to_string(),
            ],
        }
    }
}

/// 開かれている Beamer プロジェクトを管理する構造体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    /// プロジェクトのルートディレクトリ。
    pub root_dir: PathBuf,
    /// プロジェクト設定。
    pub config: ProjectConfig,
}

impl Project {
    /// 指定されたルートパスから新しいプロジェクト構造体を初期化する。
    pub fn new(root_dir: impl AsRef<Path>) -> Self {
        Self {
            root_dir: root_dir.as_ref().to_path_buf(),
            config: ProjectConfig::default(),
        }
    }

    /// メインファイルの絶対パスを取得する。
    pub fn main_file_path(&self) -> PathBuf {
        self.root_dir.join(&self.config.main_file)
    }
}
