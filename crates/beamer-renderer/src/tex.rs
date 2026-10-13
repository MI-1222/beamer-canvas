//! ローカル TeX コマンドの非同期実行・プロセス管理モジュール。

use crate::error::{RenderError, Result};
use std::path::Path;
use tokio::process::Command;

/// TeX コンパイルの実行結果を表す構造体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOutput {
    /// 終了コードが正常（0）であったかどうか。
    pub success: bool,
    /// 終了コード。
    pub exit_code: Option<i32>,
    /// 標準出力の内容。
    pub stdout: String,
    /// 標準エラー出力の内容。
    pub stderr: String,
}

/// TeX コマンドを非同期に起動・監視するランナー。
#[derive(Debug, Default, Clone)]
pub struct TexRunner;

impl TexRunner {
    /// 新しい TeXRunner インスタンスを生成する。
    pub fn new() -> Self {
        Self
    }

    /// 指定された作業ディレクトリと引数で TeX コマンドを非同期実行する。
    pub async fn run_command(
        &self,
        program: &str,
        args: &[&str],
        working_dir: &Path,
    ) -> Result<CompileOutput> {
        let output = Command::new(program)
            .args(args)
            .current_dir(working_dir)
            .output()
            .await?;

        let success = output.status.success();
        let exit_code = output.status.code();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !success {
            return Err(RenderError::CompileFailed { exit_code, stderr });
        }

        Ok(CompileOutput {
            success,
            exit_code,
            stdout,
            stderr,
        })
    }
}
