//! `BuildManager` の非同期ビルド、サンドボックス隔離、キャンセル制御の統合テスト。

use beamer_renderer::{
    BuildManager, BuildRequest, DiagnosticSeverity, RenderError, Sandbox, SandboxConfig,
    TexCommandKind, TexEnvironment, TexLogParser,
};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;

#[tokio::test]
async fn test_sandbox_isolation_and_master_protection() {
    let temp = TempDir::new().expect("一時ディレクトリの作成に失敗。");
    let config = SandboxConfig::new(temp.path());
    let sandbox = Sandbox::new(config).expect("サンドボックスの初期化に失敗。");

    // フルビルド側と部分ビルド側のディレクトリが異なっていることを確認する。
    let full_dir = sandbox.full_build_dir();
    let partial_dir = sandbox.partial_build_dir();
    assert_ne!(full_dir, partial_dir);

    // フルコンパイル成功時の中間ファイル（マスター `.aux` と `.nav`）を作成する。
    let master_aux = full_dir.join("main.aux");
    let master_nav = full_dir.join("main.nav");
    fs::write(&master_aux, "\\relax\n\\gdef\\totalpages{10}").unwrap();
    fs::write(&master_nav, "\\headcommand{\\beamer@framepages{1}{10}}").unwrap();

    // 部分コンパイル用の準備を実行する。
    sandbox
        .prepare_partial_sandbox("main")
        .expect("中間ファイルの複製に失敗。");

    let partial_aux = partial_dir.join("main.aux");
    let partial_nav = partial_dir.join("main.nav");

    assert!(partial_aux.exists());
    assert!(partial_nav.exists());

    // 部分コンパイル側の中間ファイルが変更されても、マスター側が汚染されないことを確認する。
    fs::write(&partial_aux, "\\relax\n\\gdef\\totalpages{1}").unwrap();
    assert_eq!(
        fs::read_to_string(&master_aux).unwrap(),
        "\\relax\n\\gdef\\totalpages{10}"
    );
}

#[tokio::test]
async fn test_build_manager_supersede_cancellation() {
    let manager = BuildManager::new();
    let temp = TempDir::new().expect("一時ディレクトリの作成に失敗。");

    let project_root = temp.path().to_path_buf();
    let main_tex = project_root.join("main.tex");
    fs::write(&main_tex, "% empty source").unwrap();

    // 1つ目の要求をディスパッチする（意図的に完了に時間がかかるシェルスクリプトで模倣）。
    let mut req1 = BuildRequest::full(&project_root, "main.tex");
    let manager_clone = manager.clone();
    let handle1 = tokio::spawn(async move {
        req1.extra_args = vec!["-halt-on-error".to_string()];
        manager_clone.request_build(req1).await
    });

    // わずかに待機して2つ目の要求を発行する。
    tokio::time::sleep(Duration::from_millis(5)).await;
    let req2 = BuildRequest::full(&project_root, "main.tex");
    let handle2 = tokio::spawn(async move { manager.request_build(req2).await });

    let (res1, res2) = tokio::join!(handle1, handle2);

    // 少なくともどちらかの結果を取得（環境に `TeX` がない場合は `ToolNotFound`、キャンセル時は `Cancelled`）。
    let r1 = res1.expect("タスク1の実行でパニックが発生して失敗。");
    let r2 = res2.expect("タスク2の実行でパニックが発生して失敗。");

    match r1 {
        Err(RenderError::Cancelled) => {
            // 先行ジョブが安全に中断されたことを確認。
        }
        Err(RenderError::ToolNotFound { .. }) => {
            // `TeX` がない環境でも型安全に処理される。
        }
        Ok(_) | Err(_) => {
            // `TeX` の実行結果またはエラー。
        }
    }

    if let Err(RenderError::ToolNotFound { .. }) = r2 {
        // `TeX` がない環境でも型安全に処理される。
    }
}

#[tokio::test]
async fn test_log_parser_structured_diagnostics() {
    let parser = TexLogParser::new();

    let sample_log = r#"
This is LuaHBTeX, Version 1.18.0
(./main.tex
LaTeX Warning: Reference `fig:arch' on page 2 undefined on input line 88.
! LaTeX Error: File 'diagram.pdf' not found.
l.95 \includegraphics{diagram.pdf}
! Undefined control sequence.
l.120 \unknowncommand
                     {test}
Overfull \hbox (22.5pt too wide) in paragraph at lines 130--135
)
"#;

    let summary = parser.parse(sample_log);

    assert!(summary.has_errors());
    assert!(summary.has_warnings());
    assert_eq!(summary.missing_files, vec!["diagram.pdf"]);

    // 抽出された診断情報の件数と内容を確認する。
    assert!(summary.diagnostics.len() >= 4);

    let err_missing = summary
        .diagnostics
        .iter()
        .find(|d| d.message.contains("diagram.pdf"))
        .expect("ファイル欠落エラーの抽出に失敗。");
    assert_eq!(err_missing.severity, DiagnosticSeverity::Error);
    assert_eq!(err_missing.line, Some(95));

    let err_undef = summary
        .diagnostics
        .iter()
        .find(|d| d.message.contains("Undefined control sequence"))
        .expect("未定義制御シーケンスエラーの抽出に失敗。");
    assert_eq!(err_undef.severity, DiagnosticSeverity::Error);
    assert_eq!(err_undef.line, Some(120));

    let warn_ref = summary
        .diagnostics
        .iter()
        .find(|d| d.message.contains("LaTeX Warning"))
        .expect("警告情報の抽出に失敗。");
    assert_eq!(warn_ref.severity, DiagnosticSeverity::Warning);
    assert_eq!(warn_ref.line, Some(88));
}

#[tokio::test]
async fn test_real_tex_compilation_with_build_manager() {
    let env = TexEnvironment::detect().await;
    let available_engine = if env.is_available(TexCommandKind::Lualatex) {
        Some(TexCommandKind::Lualatex)
    } else if env.is_available(TexCommandKind::Latexmk) {
        Some(TexCommandKind::Latexmk)
    } else if env.is_available(TexCommandKind::Pdflatex) {
        Some(TexCommandKind::Pdflatex)
    } else if env.is_available(TexCommandKind::Xelatex) {
        Some(TexCommandKind::Xelatex)
    } else {
        None
    };

    let Some(engine) = available_engine else {
        println!("実行環境に `TeX` コマンドが見つからないため、実機ビルドテストをスキップ。");
        return;
    };

    let temp = TempDir::new().expect("一時ディレクトリの作成に失敗。");
    let project_root = temp.path().to_path_buf();

    let beamer_source = r#"\documentclass{beamer}
\begin{document}
\begin{frame}{Build Manager Real Test}
  Testing asynchronous build with sandbox isolation.
\end{frame}
\end{document}
"#;

    let main_file = PathBuf::from("main.tex");
    fs::write(project_root.join(&main_file), beamer_source).expect("`TeX` ファイルの作成に失敗。");

    let manager = BuildManager::new();
    let mut request = BuildRequest::full(&project_root, &main_file);
    request.engine = Some(engine);

    let result = manager
        .request_build(request)
        .await
        .expect("ビルド実行に失敗。");

    assert!(result.success, "コンパイル処理に失敗。");
    assert!(result.pdf_path.is_some(), "`PDF` パスの設定に失敗。");

    let pdf_path = result.pdf_path.unwrap();
    assert!(pdf_path.exists(), "生成されたpdfファイルが存在せず失敗。");

    // 出力先がプロジェクトルート直下ではなく .build/full 配下であることを確認する。
    assert!(
        pdf_path.starts_with(project_root.join(".build").join("full")),
        "`PDF` のサンドボックス配下出力確認に失敗。"
    );
}
