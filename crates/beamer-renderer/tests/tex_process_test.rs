//! `TexRunner`による非同期実行とリアルタイムログ受信のテスト。

use beamer_renderer::{LogStream, RenderError, TexRunner};
use std::fs;
use tempfile::TempDir;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_realtime_stream_capture() {
    let runner = TexRunner::new();
    let temp_dir = TempDir::new().expect("一時ディレクトリの作成に失敗しました。");
    let (tx, mut rx) = mpsc::channel(64);

    // 標準出力と標準エラー出力の双方に複数行を出力する。
    let script = "echo 'stdout line 1'; echo 'stderr line 1' >&2; echo 'stdout line 2'; echo 'stderr line 2' >&2";
    let runner_clone = runner.clone();
    let temp_path = temp_dir.path().to_path_buf();

    let handle = tokio::spawn(async move {
        runner_clone
            .run_command_streaming("sh", &["-c", script], &temp_path, tx)
            .await
    });

    let mut stdout_lines = Vec::new();
    let mut stderr_lines = Vec::new();

    while let Some(msg) = rx.recv().await {
        match msg.stream {
            LogStream::Stdout => stdout_lines.push(msg.text),
            LogStream::Stderr => stderr_lines.push(msg.text),
        }
    }

    let result = handle
        .await
        .expect("タスク実行に失敗しました。")
        .expect("コマンド実行に成功すること。");

    assert!(result.success, "正常終了すること。");
    assert_eq!(result.exit_code, Some(0));

    assert_eq!(stdout_lines, vec!["stdout line 1", "stdout line 2"]);
    assert_eq!(stderr_lines, vec!["stderr line 1", "stderr line 2"]);
}

#[tokio::test]
async fn test_process_failure_capture() {
    let runner = TexRunner::new();
    let temp_dir = TempDir::new().expect("一時ディレクトリの作成に失敗しました。");
    let (tx, mut rx) = mpsc::channel(64);

    let script = "echo 'fatal syntax error' >&2; exit 42";
    let runner_clone = runner.clone();
    let temp_path = temp_dir.path().to_path_buf();

    let handle = tokio::spawn(async move {
        runner_clone
            .run_command_streaming("sh", &["-c", script], &temp_path, tx)
            .await
    });

    let mut captured_stderr = Vec::new();
    while let Some(msg) = rx.recv().await {
        if msg.stream == LogStream::Stderr {
            captured_stderr.push(msg.text);
        }
    }

    let result = handle.await.expect("タスク実行に失敗しました。");
    match result {
        Err(RenderError::CompileFailed {
            exit_code, stderr, ..
        }) => {
            assert_eq!(exit_code, Some(42), "終了コード42を正しく取得できること。");
            assert!(
                stderr.contains("fatal syntax error"),
                "エラーメッセージがstderrに含まれていること。"
            );
        }
        _ => panic!("CompileFailedエラーが返されること。"),
    }

    assert_eq!(captured_stderr, vec!["fatal syntax error"]);
}

#[tokio::test]
async fn test_lualatex_async_compilation_and_log_capture() {
    let runner = TexRunner::new();

    // 実行環境にlualatexまたはlatexmkが存在するか確認する。
    let lualatex_available = runner.is_program_available("lualatex").await;
    let latexmk_available = runner.is_program_available("latexmk").await;

    if !lualatex_available && !latexmk_available {
        println!("実行環境にTeXコマンドが見つからないため、実機コンパイルテストをスキップします。");
        return;
    }

    let temp_dir = TempDir::new().expect("一時ディレクトリの作成に失敗しました。");
    let tex_source = r#"\documentclass{beamer}
\begin{document}
\begin{frame}{Async Compile Test}
  Testing real-time log capture.
\end{frame}
\end{document}
"#;

    let tex_path = temp_dir.path().join("main.tex");
    fs::write(&tex_path, tex_source).expect("TeXソースファイルの書き込みに失敗しました。");

    let (tx, mut rx) = mpsc::channel(128);
    let (program, args) = if lualatex_available {
        ("lualatex", vec!["-interaction=nonstopmode", "main.tex"])
    } else {
        (
            "latexmk",
            vec!["-pdf", "-interaction=nonstopmode", "main.tex"],
        )
    };

    let runner_clone = runner.clone();
    let workdir = temp_dir.path().to_path_buf();

    let compile_task = tokio::spawn(async move {
        runner_clone
            .run_command_streaming(program, &args, &workdir, tx)
            .await
    });

    let mut log_count = 0;
    while let Some(_msg) = rx.recv().await {
        log_count += 1;
    }

    let output = compile_task
        .await
        .expect("非同期タスクが完了すること。")
        .expect("TeXコンパイルが成功すること。");

    assert!(output.success, "コンパイルが正常終了すること。");
    assert!(
        log_count > 0,
        "リアルタイムログが1行以上受信されていること。"
    );

    // 出力PDFが生成されていることを確認する。
    let output_pdf = temp_dir.path().join("main.pdf");
    assert!(output_pdf.exists(), "main.pdfが生成されていること。");
}
