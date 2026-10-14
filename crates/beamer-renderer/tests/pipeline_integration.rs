//! `Beamer`ドキュメントのコンパイルから画像化までの一連のパイプライン統合テスト。
//!
//! ソースコードの入力、サンドボックスでのビルド、中間ファイル(`.nav`)の解析、
//! ページマッピングテーブルの構築、および`PDFium`による生`RGBA`ビットマップ取得までの
//! 一気通貫の連携動作を検証する。

use beamer_renderer::{
    BuildManager, BuildRequest, DiagnosticSeverity, PdfRasterizer, RenderOptions, Sandbox,
    SandboxConfig, TexCommandKind, TexEnvironment,
};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// 利用可能な `TeX` エンジンを検出する。見つからない場合は `None` を返却。
async fn detect_available_engine() -> Option<TexCommandKind> {
    let env = TexEnvironment::detect().await;
    if env.is_available(TexCommandKind::Lualatex) {
        Some(TexCommandKind::Lualatex)
    } else if env.is_available(TexCommandKind::Latexmk) {
        Some(TexCommandKind::Latexmk)
    } else if env.is_available(TexCommandKind::Pdflatex) {
        Some(TexCommandKind::Pdflatex)
    } else if env.is_available(TexCommandKind::Xelatex) {
        Some(TexCommandKind::Xelatex)
    } else {
        None
    }
}

#[tokio::test]
async fn test_e2e_pipeline_full_compilation_and_rendering() {
    let Some(engine) = detect_available_engine().await else {
        println!("実行環境に`TeX`コマンドが見つからないため、パイプライン統合テストをスキップ。");
        return;
    };

    let Ok(rasterizer) = PdfRasterizer::new() else {
        println!("`PDFium`ライブラリが見つからないため、パイプライン統合テストをスキップ。");
        return;
    };

    let temp_dir = TempDir::new().expect("一時ディレクトリの作成に失敗。");
    let project_root = temp_dir.path().to_path_buf();

    // 2フレーム構成: フレーム1は静的スライド(1ページ)、フレーム2は箇条書きとオーバーレイ(2ページ)。
    let beamer_source = r#"\documentclass{beamer}
\usetheme{default}
\begin{document}

\begin{frame}{First Static Frame}
  This is the first static slide content.
  \begin{block}{Notice}
    Important block message.
  \end{block}
\end{frame}

\begin{frame}{Second Animated Frame}
  First point visible immediately.
  \pause
  Second point visible after pause.
\end{frame}

\end{document}
"#;

    let main_file = PathBuf::from("main.tex");
    fs::write(project_root.join(&main_file), beamer_source)
        .expect("`TeX`ソースファイルの書き込みに失敗。");

    let manager = BuildManager::new();
    let mut request = BuildRequest::full(&project_root, &main_file);
    request.engine = Some(engine);

    // 1. バックグラウンドビルドの実行。
    let result = manager
        .request_build(request)
        .await
        .expect("ビルド実行処理が原因で失敗しました。");

    assert!(result.success, "コンパイル処理に失敗。");
    assert!(result.pdf_path.is_some(), "`PDF`パスの設定に失敗。");
    let pdf_path = result.pdf_path.as_ref().unwrap();
    assert!(pdf_path.exists(), "生成された`PDF`ファイルが存在せず失敗。");

    // 2. `.nav`中間ファイルおよびページマッピングテーブルの検証。
    assert!(
        result.nav_content.is_some(),
        "中間ファイル`.nav`の読み込みに失敗。"
    );
    let mapping_table = result
        .mapping_table_with_fallback(&rasterizer, Some(2))
        .expect("ページマッピングテーブルの構築に失敗。");

    assert_eq!(mapping_table.frame_count(), 2, "フレーム数の抽出に失敗。");
    assert_eq!(
        mapping_table.total_physical_pages(),
        3,
        "総物理ページ数の計算に失敗。"
    );

    // 各フレームおよびステップと物理ページ番号(0始まり)の対応確認。
    // フレーム0: ステップ0 -> 物理ページ0。
    assert_eq!(mapping_table.get_physical_page(0, 0), Some(0));
    // フレーム1: ステップ0 -> 物理ページ1。
    assert_eq!(mapping_table.get_physical_page(1, 0), Some(1));
    // フレーム1: ステップ1 -> 物理ページ2。
    assert_eq!(mapping_table.get_physical_page(1, 1), Some(2));

    // 物理ページからフレームおよびステップへの逆引き整合性確認。
    assert_eq!(mapping_table.lookup_frame_and_step(0), Some((0, 0)));
    assert_eq!(mapping_table.lookup_frame_and_step(1), Some((1, 0)));
    assert_eq!(mapping_table.lookup_frame_and_step(2), Some((1, 1)));

    // 3. `PDFium`による指定ページのラスタライズとビットマップ整合性検証。
    let options_150dpi = RenderOptions::new().with_dpi(150);

    // フレーム0(物理ページ0)のレンダリング。
    let target_page_f0 = mapping_table.get_physical_page(0, 0).unwrap();
    let bitmap_f0 = rasterizer
        .render_page_to_rgba_with_options(pdf_path, target_page_f0, &options_150dpi)
        .expect("フレーム0のラスタライズに失敗。");

    assert_eq!(bitmap_f0.page_index, 0);
    assert!(bitmap_f0.width > 0, "画像幅の取得に失敗。");
    assert!(bitmap_f0.height > 0, "画像高さの取得に失敗。");
    let expected_bytes = (bitmap_f0.width * bitmap_f0.height * 4) as usize;
    assert_eq!(
        bitmap_f0.rgba_data.len(),
        expected_bytes,
        "ピクセルバイト長の検証に失敗。"
    );

    // フレーム1ステップ0(物理ページ1)のレンダリング。
    let target_page_f1_s0 = mapping_table.get_physical_page(1, 0).unwrap();
    let bitmap_f1_s0 = rasterizer
        .render_page_to_rgba_with_options(pdf_path, target_page_f1_s0, &options_150dpi)
        .expect("フレーム1ステップ0のラスタライズに失敗。");

    // フレーム1ステップ1(物理ページ2)のレンダリング。
    let target_page_f1_s1 = mapping_table.get_physical_page(1, 1).unwrap();
    let bitmap_f1_s1 = rasterizer
        .render_page_to_rgba_with_options(pdf_path, target_page_f1_s1, &options_150dpi)
        .expect("フレーム1ステップ1のラスタライズに失敗。");

    // アニメーションステップの進行に伴うビットマップ差分の確認。
    assert_ne!(
        bitmap_f1_s0.rgba_data, bitmap_f1_s1.rgba_data,
        "ステップ間の描画差分検知に失敗。"
    );
}

#[tokio::test]
async fn test_e2e_partial_compilation_and_master_protection() {
    let Some(engine) = detect_available_engine().await else {
        println!("実行環境に`TeX`コマンドが見つからないため、部分コンパイルテストをスキップ。");
        return;
    };

    let Ok(rasterizer) = PdfRasterizer::new() else {
        println!("`PDFium`ライブラリが見つからないため、部分コンパイルテストをスキップ。");
        return;
    };

    let temp_dir = TempDir::new().expect("一時ディレクトリの作成に失敗。");
    let project_root = temp_dir.path().to_path_buf();

    let beamer_source = r#"\documentclass{beamer}
\begin{document}
\begin{frame}[label=first]{Slide One}
  Content of first slide.
\end{frame}
\begin{frame}[label=second]{Slide Two}
  Content of second slide.
\end{frame}
\end{document}
"#;

    let main_file = PathBuf::from("main.tex");
    fs::write(project_root.join(&main_file), beamer_source)
        .expect("`TeX`ファイルの書き込みに失敗。");

    let manager = BuildManager::new();

    // 1. 初回のフルビルド実行。
    let mut full_req = BuildRequest::full(&project_root, &main_file);
    full_req.engine = Some(engine);
    let full_result = manager
        .request_build(full_req)
        .await
        .expect("フルビルド実行に失敗。");
    assert!(full_result.success, "フルビルド処理に失敗。");

    let sandbox_config = SandboxConfig::new(&project_root);
    let sandbox = Sandbox::new(sandbox_config).expect("サンドボックスの初期化に失敗。");
    let master_nav_path = sandbox.full_build_dir().join("main.nav");
    assert!(master_nav_path.exists(), "マスター`.nav`の確認に失敗。");
    let master_nav_content =
        fs::read_to_string(&master_nav_path).expect("マスター`.nav`の読み込みに失敗。");

    // 2. 特定スライドの部分ビルド実行。
    let mut partial_req =
        BuildRequest::partial(&project_root, &main_file, Some("second".to_string()));
    partial_req.engine = Some(engine);
    let partial_result = manager
        .request_build(partial_req)
        .await
        .expect("部分ビルド実行に失敗。");

    assert!(partial_result.success, "部分コンパイル処理に失敗。");

    // マスター中間ファイルの非破壊確認。
    let current_master_nav =
        fs::read_to_string(&master_nav_path).expect("マスター`.nav`の再読み込みに失敗。");
    assert_eq!(
        master_nav_content, current_master_nav,
        "マスター中間ファイル保持の確認に失敗。"
    );

    // 部分コンパイルで出力された`PDF`の描画確認。
    let partial_pdf = partial_result
        .pdf_path
        .expect("部分コンパイル`PDF`の出力確認に失敗。");
    let page_count = rasterizer
        .get_page_count_from_file(&partial_pdf)
        .expect("ページ数取得に失敗。");
    assert_eq!(page_count, 1, "部分コンパイル結果のページ数判定に失敗。");

    let options = RenderOptions::new().with_dpi(150);
    let partial_bitmap = rasterizer
        .render_page_to_rgba_with_options(&partial_pdf, 0, &options)
        .expect("部分コンパイル`PDF`のラスタライズに失敗。");
    assert!(
        partial_bitmap.width > 0,
        "部分コンパイル画像幅の取得に失敗。"
    );
    assert!(
        partial_bitmap.height > 0,
        "部分コンパイル画像高さの取得に失敗。"
    );
}

#[tokio::test]
async fn test_pipeline_diagnostics_on_compilation_error() {
    let Some(engine) = detect_available_engine().await else {
        println!("実行環境に`TeX`コマンドが見つからないため、エラー診断テストをスキップ。");
        return;
    };

    let temp_dir = TempDir::new().expect("一時ディレクトリの作成に失敗。");
    let project_root = temp_dir.path().to_path_buf();

    // 未定義制御シーケンスを含む構文エラーコード。
    let broken_source = r#"\documentclass{beamer}
\begin{document}
\begin{frame}{Syntax Error Test}
  Normal text before error.
  \undefinedControlSequenceForErrorTesting{broken}
\end{frame}
\end{document}
"#;

    let main_file = PathBuf::from("main.tex");
    fs::write(project_root.join(&main_file), broken_source)
        .expect("`TeX`ファイルの書き込みに失敗。");

    let manager = BuildManager::new();
    let mut request = BuildRequest::full(&project_root, &main_file);
    request.engine = Some(engine);

    let result = manager
        .request_build(request)
        .await
        .expect("ビルド実行が原因で失敗しました。");

    // ビルド失敗時の構造化診断情報返却確認。
    assert!(!result.success, "構文エラーに対するビルド失敗判定に失敗。");

    let error_diagnostics: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .collect();

    assert!(
        !error_diagnostics.is_empty(),
        "エラー診断情報の抽出に失敗。"
    );

    let found_undefined_macro = error_diagnostics.iter().any(|d| {
        d.message.contains("Undefined control sequence")
            || d.message.contains("undefinedControlSequence")
    });
    assert!(
        found_undefined_macro,
        "未定義コマンドの診断メッセージ抽出に失敗。"
    );
}
