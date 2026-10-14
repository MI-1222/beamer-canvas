//! 非同期ビルドマネージャー。
//!
//! 連続した編集要求に対する先行プロセスの安全なキャンセル（`Supersede` 制御）、
//! サンドボックスと中間ファイルの分離保護、および `TeX` ログの構造化診断を統合して実行する。

use super::detector::{TexCommandKind, TexEnvironment};
use super::log_parser::{Diagnostic, TexLogParser};
use super::runner::{LogMessage, TexRunner};
use super::sandbox::{BuildKind, Sandbox, SandboxConfig};
use crate::error::{RenderError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc::Sender};
use tokio_util::sync::CancellationToken;

/// ビルド要求のパラメータを保持する構造体。
#[derive(Debug, Clone)]
pub struct BuildRequest {
    /// プロジェクトのルートディレクトリ。
    pub project_root: PathBuf,
    /// メインとなる `TeX` ファイルの相対パス（例: `main.tex`）。
    pub main_file: PathBuf,
    /// ビルド種別（フルコンパイルまたは部分コンパイル）。
    pub kind: BuildKind,
    /// 使用する `TeX` エンジン（未指定時はシステム環境から自動選択）。
    pub engine: Option<TexCommandKind>,
    /// 追加のコンパイルオプション引数。
    pub extra_args: Vec<String>,
    /// メモリ上のソースコード文字列（未保存の入力内容や一時ラベル注入版）。
    pub source_override: Option<String>,
}

impl BuildRequest {
    /// フルコンパイル要求を新規作成する。
    pub fn full(project_root: impl AsRef<Path>, main_file: impl AsRef<Path>) -> Self {
        Self {
            project_root: project_root.as_ref().to_path_buf(),
            main_file: main_file.as_ref().to_path_buf(),
            kind: BuildKind::Full,
            engine: None,
            extra_args: Vec::new(),
            source_override: None,
        }
    }

    /// 部分コンパイル要求を新規作成する。
    pub fn partial(
        project_root: impl AsRef<Path>,
        main_file: impl AsRef<Path>,
        frame_label: Option<String>,
    ) -> Self {
        Self {
            project_root: project_root.as_ref().to_path_buf(),
            main_file: main_file.as_ref().to_path_buf(),
            kind: BuildKind::Partial { frame_label },
            engine: None,
            extra_args: Vec::new(),
            source_override: None,
        }
    }
}

/// ビルドの実行結果を保持する構造体。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResult {
    /// ビルドが正常終了し有効なpdfが出力されたかどうか。
    pub success: bool,
    /// プロセスの終了コード。
    pub exit_code: Option<i32>,
    /// 生成されたpdfファイルの絶対パス。
    pub pdf_path: Option<PathBuf>,
    /// 抽出された診断情報（エラーや警告）の一覧。
    pub diagnostics: Vec<Diagnostic>,
    /// 検知された不足ファイル名の一覧。
    pub missing_files: Vec<String>,
    /// 中間ファイル `.nav` の内容（オーバーレイとページ番号の同期用）。
    pub nav_content: Option<String>,
    /// 標準出力ログの全テキスト。
    pub stdout: String,
    /// 標準エラー出力ログの全テキスト。
    pub stderr: String,
}

/// 内部状態（実行中タスクのキャンセルトークン）を保持するスロット。
#[derive(Debug, Default)]
struct BuildSlot {
    current_token: Option<CancellationToken>,
}

/// 非同期ビルドマネージャー。
#[derive(Debug, Clone)]
pub struct BuildManager {
    /// 低レイヤープロセス実行器。
    runner: TexRunner,
    /// ログ解析器。
    parser: TexLogParser,
    /// 実行中タスクのトークン状態。
    slot: Arc<Mutex<BuildSlot>>,
    /// ビルドプロセスの排他実行用ロック。
    execution_lock: Arc<Mutex<()>>,
}

impl Default for BuildManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildManager {
    /// 新しいビルドマネージャーを生成する。
    pub fn new() -> Self {
        Self {
            runner: TexRunner::new(),
            parser: TexLogParser::new(),
            slot: Arc::new(Mutex::new(BuildSlot::default())),
            execution_lock: Arc::new(Mutex::new(())),
        }
    }

    /// ビルド要求を発行し、完了または中断まで待機する。
    ///
    /// 実行中に新しい要求が到着した場合、先行プロセスは安全に停止される。
    pub async fn request_build(&self, request: BuildRequest) -> Result<BuildResult> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(128);
        tokio::spawn(async move { while rx.recv().await.is_some() {} });
        self.request_build_streaming(request, tx).await
    }

    /// ビルド要求を発行し、ログをストリーミングしながら完了まで待機する。
    pub async fn request_build_streaming(
        &self,
        request: BuildRequest,
        tx: Sender<LogMessage>,
    ) -> Result<BuildResult> {
        // 1. 先行ジョブのキャンセルと新規トークンの登録を行う。
        let my_token = {
            let mut slot_guard = self.slot.lock().await;
            if let Some(old_token) = slot_guard.current_token.take() {
                old_token.cancel();
            }
            let token = CancellationToken::new();
            slot_guard.current_token = Some(token.clone());
            token
        };

        // 2. 実行ロックを取得し、先行プロセスが確実に終了してディスクロックが解放されるのを待機する。
        let _exec_guard = self.execution_lock.lock().await;

        // 待機中に後続要求によってすでにキャンセルされている場合は即座に返る。
        if my_token.is_cancelled() {
            return Err(RenderError::Cancelled);
        }

        // 3. サンドボックス環境の準備。
        let sandbox_config = SandboxConfig::new(&request.project_root);
        let sandbox = Sandbox::new(sandbox_config)?;

        let main_file_name = request
            .main_file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("main.tex");
        let stem = request
            .main_file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("main");

        // 部分コンパイル時は、直前のマスター中間ファイルを複製する。
        if matches!(request.kind, BuildKind::Partial { .. }) {
            let _ = sandbox.prepare_partial_sandbox(stem);
        }

        let workdir = sandbox.output_dir(&request.kind).to_path_buf();
        let texinputs = sandbox.build_texinputs(&workdir);
        let env_vars = [("TEXINPUTS", texinputs.as_str())];

        // 4. コンパイル対象ファイルの決定（インメモリテキストの書き出し対応）。
        let target_file_path = if let Some(ref source) = request.source_override {
            let temp_source_path = workdir.join(main_file_name);
            fs::write(&temp_source_path, source).map_err(|e| RenderError::Sandbox {
                message: format!("一時ソースファイルの書き込みに失敗しました: {e}"),
            })?;
            temp_source_path
        } else {
            request.project_root.join(&request.main_file)
        };

        // 5. 使用エンジンの決定。
        let engine_kind = match request.engine {
            Some(k) => k,
            None => self.choose_engine(&request.kind).await?,
        };

        let program_name = engine_kind.as_str();
        let target_file_str = target_file_path.to_string_lossy().to_string();
        let outdir_str = workdir.to_string_lossy().to_string();

        // 6. コマンドライン引数の構築。
        let args = self.build_command_args(
            engine_kind,
            &outdir_str,
            &target_file_str,
            &request.extra_args,
        );
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        // 7. キャンセル可能なプロセスグループ実行。
        let compile_res = self
            .runner
            .run_command_streaming_cancellable(
                program_name,
                &arg_refs,
                &request.project_root,
                &env_vars,
                tx,
                my_token.clone(),
            )
            .await;

        let (output_success, exit_code, stdout, stderr) = match compile_res {
            Ok(output) => (
                output.success,
                output.exit_code,
                output.stdout,
                output.stderr,
            ),
            Err(RenderError::Cancelled) => return Err(RenderError::Cancelled),
            Err(RenderError::CompileFailed { exit_code, stderr }) => {
                (false, exit_code, String::new(), stderr)
            }
            Err(e) => return Err(e),
        };

        // 8. ログ解析と診断情報の抽出。
        let full_log = format!("{}\n{}", stdout, stderr);
        let log_summary = self.parser.parse(&full_log);

        let pdf_path = sandbox.get_pdf_path(&request.kind, stem);
        let pdf_exists = pdf_path.exists();

        // 総合判定: 終了コード正常、`PDF` 生成済み、エラー診断情報なしの場合に成功とする。
        let overall_success = output_success && pdf_exists && !log_summary.has_errors();

        // フルコンパイル成功時は、オーバーレイ同期用に `.nav` の内容を読み出す。
        let nav_content = if overall_success && matches!(request.kind, BuildKind::Full) {
            let nav_path = sandbox.get_master_nav_path(stem);
            fs::read_to_string(nav_path).ok()
        } else {
            None
        };

        Ok(BuildResult {
            success: overall_success,
            exit_code,
            pdf_path: if pdf_exists { Some(pdf_path) } else { None },
            diagnostics: log_summary.diagnostics,
            missing_files: log_summary.missing_files,
            nav_content,
            stdout,
            stderr,
        })
    }

    /// 現在のシステム環境から適切な `TeX` エンジンを選択する。
    ///
    /// 部分コンパイル時は、高速化と孫プロセス防止のため直接エンジンを優先する。
    async fn choose_engine(&self, kind: &BuildKind) -> Result<TexCommandKind> {
        let env = TexEnvironment::detect().await;

        match kind {
            BuildKind::Partial { .. } => {
                // 部分コンパイル時は直接エンジンを最優先する。
                if env.is_available(TexCommandKind::Lualatex) {
                    Ok(TexCommandKind::Lualatex)
                } else if env.is_available(TexCommandKind::Xelatex) {
                    Ok(TexCommandKind::Xelatex)
                } else if env.is_available(TexCommandKind::Pdflatex) {
                    Ok(TexCommandKind::Pdflatex)
                } else if env.is_available(TexCommandKind::Latexmk) {
                    Ok(TexCommandKind::Latexmk)
                } else {
                    Err(RenderError::ToolNotFound {
                        command: "lualatex / xelatex / pdflatex / latexmk".to_string(),
                    })
                }
            }
            BuildKind::Full => {
                // フルコンパイル時はリラン自動化の観点から latexmk を優先する。
                if env.is_available(TexCommandKind::Latexmk) {
                    Ok(TexCommandKind::Latexmk)
                } else if env.is_available(TexCommandKind::Lualatex) {
                    Ok(TexCommandKind::Lualatex)
                } else if env.is_available(TexCommandKind::Xelatex) {
                    Ok(TexCommandKind::Xelatex)
                } else if env.is_available(TexCommandKind::Pdflatex) {
                    Ok(TexCommandKind::Pdflatex)
                } else {
                    Err(RenderError::ToolNotFound {
                        command: "latexmk / lualatex / xelatex / pdflatex".to_string(),
                    })
                }
            }
        }
    }

    /// コマンドライン引数を構築する。
    fn build_command_args(
        &self,
        engine: TexCommandKind,
        output_dir: &str,
        target_file: &str,
        extra_args: &[String],
    ) -> Vec<String> {
        let mut args = Vec::new();

        match engine {
            TexCommandKind::Latexmk => {
                args.push("-pdf".to_string());
                args.push("-interaction=nonstopmode".to_string());
                args.push("-halt-on-error".to_string());
                args.push(format!("-outdir={output_dir}"));
            }
            TexCommandKind::Lualatex | TexCommandKind::Xelatex | TexCommandKind::Pdflatex => {
                args.push("-interaction=nonstopmode".to_string());
                args.push("-halt-on-error".to_string());
                args.push(format!("-output-directory={output_dir}"));
            }
        }

        for extra in extra_args {
            args.push(extra.clone());
        }

        args.push(target_file.to_string());
        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_cancellation_of_stale_build() {
        let _manager = BuildManager::new();
        let temp = TempDir::new().expect("一時ディレクトリの作成に失敗。");

        let project_root = temp.path().to_path_buf();

        let (tx, mut rx) = tokio::sync::mpsc::channel(64);
        let cancel_token = CancellationToken::new();
        let token_clone = cancel_token.clone();

        let handle = tokio::spawn(async move {
            let runner = TexRunner::new();
            runner
                .run_command_streaming_cancellable(
                    "sh",
                    &["-c", "sleep 10"],
                    &project_root,
                    &[],
                    tx,
                    token_clone,
                )
                .await
        });

        // 100ms後にキャンセルを発火する。
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        cancel_token.cancel();

        let res = handle.await.unwrap();
        match res {
            Err(RenderError::Cancelled) => {
                // 正常にキャンセルを検知。
            }
            other => panic!("キャンセル処理に失敗: {:?}", other),
        }

        while rx.recv().await.is_some() {}
    }

    #[tokio::test]
    async fn test_build_args_construction() {
        let manager = BuildManager::new();

        let args_latexmk = manager.build_command_args(
            TexCommandKind::Latexmk,
            ".build/full",
            "main.tex",
            &["-synctex=1".to_string()],
        );
        assert!(args_latexmk.contains(&"-pdf".to_string()));
        assert!(args_latexmk.contains(&"-outdir=.build/full".to_string()));
        assert!(args_latexmk.contains(&"-synctex=1".to_string()));
        assert_eq!(args_latexmk.last().unwrap(), "main.tex");

        let args_lualatex =
            manager.build_command_args(TexCommandKind::Lualatex, ".build/partial", "part.tex", &[]);
        assert!(args_lualatex.contains(&"-output-directory=.build/partial".to_string()));
        assert_eq!(args_lualatex.last().unwrap(), "part.tex");
    }
}
