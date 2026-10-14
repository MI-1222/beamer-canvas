//! ローカル環境における `TeX` コマンド実行とプロセス管理を行うモジュール。

use crate::error::{RenderError, Result};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::Sender;

use super::detector::TexEnvironment;

/// プロセスの標準出力または標準エラー出力を表す列挙型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStream {
    /// 標準出力。
    Stdout,
    /// 標準エラー出力。
    Stderr,
}

/// 子プロセスの出力を1行単位で保持する構造体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogMessage {
    /// 出力元ストリーム。
    pub stream: LogStream,
    /// 1行分のテキスト。
    pub text: String,
}

/// `TeX` コマンドの実行結果を保持する構造体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOutput {
    /// 終了コードが `0`(正常終了)であったかどうか。
    pub success: bool,
    /// プロセスの終了コード。
    pub exit_code: Option<i32>,
    /// 標準出力の全テキスト。
    pub stdout: String,
    /// 標準エラー出力の全テキスト。
    pub stderr: String,
}

/// `TeX` コマンドを非同期に実行するランナー構造体。
#[derive(Debug, Default, Clone)]
pub struct TexRunner;

impl TexRunner {
    /// 新しいインスタンスを生成する。
    pub fn new() -> Self {
        Self
    }

    /// 指定した作業ディレクトリと引数で `TeX` コマンドを非同期実行する。
    pub async fn run_command(
        &self,
        program: &str,
        args: &[&str],
        working_dir: &Path,
    ) -> Result<CompileOutput> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(128);
        tokio::spawn(async move { while rx.recv().await.is_some() {} });
        self.run_command_streaming(program, args, working_dir, tx)
            .await
    }

    /// コマンドを非同期実行し、標準出力と標準エラー出力を1行ずつチャンネルへ送信しながら完了を待機する。
    pub async fn run_command_streaming(
        &self,
        program: &str,
        args: &[&str],
        working_dir: &Path,
        tx: Sender<LogMessage>,
    ) -> Result<CompileOutput> {
        self.run_command_streaming_cancellable(
            program,
            args,
            working_dir,
            &[],
            tx,
            tokio_util::sync::CancellationToken::new(),
        )
        .await
    }

    /// `CancellationToken` を監視しながらコマンドを非同期実行する。
    ///
    /// プロセスグループを生成して起動し、キャンセル時には子孫プロセスを含めて安全に強制終了する。
    pub async fn run_command_streaming_cancellable(
        &self,
        program: &str,
        args: &[&str],
        working_dir: &Path,
        env_vars: &[(&str, &str)],
        tx: Sender<LogMessage>,
        cancel_token: tokio_util::sync::CancellationToken,
    ) -> Result<CompileOutput> {
        use command_group::AsyncCommandGroup;

        let mut cmd = Command::new(program);
        cmd.args(args)
            .current_dir(working_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, val) in env_vars {
            cmd.env(key, val);
        }

        let mut child = cmd.group_spawn()?;

        let stdout = child.inner().stdout.take();
        let stderr = child.inner().stderr.take();

        let tx_stdout = tx.clone();
        let stdout_task = tokio::spawn(async move {
            let mut captured = Vec::new();
            if let Some(stdout_pipe) = stdout {
                let reader = BufReader::new(stdout_pipe);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx_stdout
                        .send(LogMessage {
                            stream: LogStream::Stdout,
                            text: line.clone(),
                        })
                        .await;
                    captured.push(line);
                }
            }
            captured.join("\n")
        });

        let tx_stderr = tx;
        let stderr_task = tokio::spawn(async move {
            let mut captured = Vec::new();
            if let Some(stderr_pipe) = stderr {
                let reader = BufReader::new(stderr_pipe);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx_stderr
                        .send(LogMessage {
                            stream: LogStream::Stderr,
                            text: line.clone(),
                        })
                        .await;
                    captured.push(line);
                }
            }
            captured.join("\n")
        });

        tokio::select! {
            biased;

            _ = cancel_token.cancelled() => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                stdout_task.abort();
                stderr_task.abort();
                Err(RenderError::Cancelled)
            }

            status_res = child.wait() => {
                let status = status_res?;
                let stdout_str = stdout_task.await.unwrap_or_default();
                let stderr_str = stderr_task.await.unwrap_or_default();

                let success = status.success();
                let exit_code = status.code();

                if !success {
                    return Err(RenderError::CompileFailed {
                        exit_code,
                        stderr: stderr_str,
                    });
                }

                Ok(CompileOutput {
                    success,
                    exit_code,
                    stdout: stdout_str,
                    stderr: stderr_str,
                })
            }
        }
    }

    /// 指定した実行可能ファイルがシステム上に存在するか確認する。
    pub async fn is_program_available(&self, program: &str) -> bool {
        Command::new(program)
            .arg("--version")
            .output()
            .await
            .is_ok_and(|output| output.status.success())
    }

    /// システム上の `TeX` 環境を自動検出する。
    pub async fn detect_environment(&self) -> TexEnvironment {
        TexEnvironment::detect().await
    }
}
