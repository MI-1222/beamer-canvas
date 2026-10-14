//! `PdfRasterizer` のビットマップ変換、レンダリングオプション、
//! メタデータ取得、およびマルチスレッド並行安全性のテスト。

use beamer_renderer::{
    BuildManager, BuildRequest, PdfRasterizer, RenderOptions, TexCommandKind, TexEnvironment,
};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

/// テスト用の最小限の有効な PDF バイト列(幅 300pt, 高さ 200pt、1 ページ)。
const MINIMAL_PDF: &[u8] = b"%PDF-1.4
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources <<>> >> endobj
xref
0 4
0000000000 65535 f 
0000000009 00000 n 
0000000058 00000 n 
0000000115 00000 n 
trailer << /Size 4 /Root 1 0 R >>
startxref
206
%%EOF
";

#[test]
fn test_pdf_rasterizer_metadata_and_dimensions() {
    let Ok(rasterizer) = PdfRasterizer::new() else {
        println!("PDFiumライブラリが見つからないためテストをスキップ。");
        return;
    };

    let page_count = rasterizer
        .get_page_count(MINIMAL_PDF)
        .expect("ページ数取得に失敗。");
    assert_eq!(page_count, 1, "最小 PDF は 1 ページであること。");

    let dims = rasterizer
        .get_page_dimensions(MINIMAL_PDF, 0)
        .expect("ページ寸法取得に失敗。");
    assert_eq!(dims.width_pt, 300.0);
    assert_eq!(dims.height_pt, 200.0);

    let labels = rasterizer
        .get_page_labels(MINIMAL_PDF)
        .expect("ラベル一覧取得に失敗。");
    assert_eq!(labels.len(), 1);
}

#[test]
fn test_pdf_rasterizer_options_and_scaling() {
    let Ok(rasterizer) = PdfRasterizer::new() else {
        println!("PDFiumライブラリが見つからないためテストをスキップ。");
        return;
    };

    // 1. 標準 72 DPI(等倍: 幅 300 px, 高さ 200 px)
    let options_72 = RenderOptions::new().with_dpi(72);
    let bitmap_72 = rasterizer
        .render_page_from_bytes(MINIMAL_PDF, 0, &options_72)
        .expect("72 DPI レンダリングに失敗。");
    assert_eq!(bitmap_72.width, 300);
    assert_eq!(bitmap_72.height, 200);
    assert_eq!(bitmap_72.rgba_data.len(), (300 * 200 * 4) as usize);

    // 白背景(ホワイトバック）が適用されていること（RGBA の先頭ピクセルが (255, 255, 255, 255))
    assert_eq!(
        &bitmap_72.rgba_data[0..4],
        &[255, 255, 255, 255],
        "白背景が強制適用されていること。"
    );

    // 2. 144 DPI(2 倍スケール: 幅 600 px, 高さ 400 px)
    let options_144 = RenderOptions::new().with_dpi(144);
    let bitmap_144 = rasterizer
        .render_page_from_bytes(MINIMAL_PDF, 0, &options_144)
        .expect("144 DPI レンダリングに失敗。");
    assert_eq!(bitmap_144.width, 600);
    assert_eq!(bitmap_144.height, 400);

    // 3. ズーム倍率(72 DPI × 1.5 ズーム: 幅 450 px, 高さ 300 px)
    let options_zoom = RenderOptions::new().with_dpi(72).with_zoom(1.5);
    let bitmap_zoom = rasterizer
        .render_page_from_bytes(MINIMAL_PDF, 0, &options_zoom)
        .expect("ズームレンダリングに失敗。");
    assert_eq!(bitmap_zoom.width, 450);
    assert_eq!(bitmap_zoom.height, 300);

    // 4. 目標サイズ指定(幅 800 px, 高さ 600 px)
    let options_fit = RenderOptions::new().fit_to(800, 600);
    let bitmap_fit = rasterizer
        .render_page_from_bytes(MINIMAL_PDF, 0, &options_fit)
        .expect("目標サイズレンダリングに失敗。");
    assert_eq!(bitmap_fit.width, 800);
    assert_eq!(bitmap_fit.height, 600);
}

#[test]
fn test_pdf_rasterizer_thread_safety_concurrent_access() {
    let Ok(rasterizer) = PdfRasterizer::new() else {
        println!("PDFiumライブラリが見つからないためテストをスキップ。");
        return;
    };
    let rasterizer = Arc::new(rasterizer);
    let mut handles = Vec::new();

    // 複数スレッドから同時にレンダリングを実行しても競合やクラッシュを起こさないことを確認する。
    for i in 0..8 {
        let r_clone = Arc::clone(&rasterizer);
        let handle = std::thread::spawn(move || {
            let options = RenderOptions::new().with_dpi(72 + (i as u16 * 10));
            let bitmap = r_clone
                .render_page_from_bytes(MINIMAL_PDF, 0, &options)
                .expect("マルチスレッドレンダリングに失敗。");
            assert!(bitmap.width > 0);
            assert!(bitmap.height > 0);
            assert_eq!(
                bitmap.rgba_data.len(),
                (bitmap.width * bitmap.height * 4) as usize
            );
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("スレッドの完了待機に失敗。");
    }
}

#[tokio::test]
async fn test_real_beamer_pdf_rendering_and_mapping_integration() {
    let env = TexEnvironment::detect().await;
    let available_engine = if env.is_available(TexCommandKind::Lualatex) {
        Some(TexCommandKind::Lualatex)
    } else if env.is_available(TexCommandKind::Latexmk) {
        Some(TexCommandKind::Latexmk)
    } else if env.is_available(TexCommandKind::Pdflatex) {
        Some(TexCommandKind::Pdflatex)
    } else {
        None
    };

    let Some(engine) = available_engine else {
        println!("TeX エンジンが存在しないため、実機 Beamer レンダリング結合テストをスキップ。");
        return;
    };

    let Ok(rasterizer) = PdfRasterizer::new() else {
        println!(
            "PDFiumライブラリが存在しないため、実機 Beamer レンダリング結合テストをスキップ。"
        );
        return;
    };

    let temp = TempDir::new().expect("一時ディレクトリの作成に失敗。");
    let project_root = temp.path().to_path_buf();

    // 2 フレーム、第 2 フレームに 2 ステップ(\pause)を持つ Beamer ソース
    let beamer_source = r#"\documentclass{beamer}
\begin{document}
\begin{frame}{Slide One}
  Static content.
\end{frame}
\begin{frame}{Slide Two with Pause}
  Step 1 visible.
  \pause
  Step 2 visible.
\end{frame}
\end{document}
"#;

    let main_file = PathBuf::from("main.tex");
    std::fs::write(project_root.join(&main_file), beamer_source)
        .expect("`TeX` ファイルの作成に失敗。");

    let manager = BuildManager::new();
    let mut request = BuildRequest::full(&project_root, &main_file);
    request.engine = Some(engine);

    let result = manager
        .request_build(request)
        .await
        .expect("ビルド実行に失敗。");

    assert!(result.success, "コンパイルに成功すること。");
    let pdf_path = result.pdf_path.as_ref().expect("PDF が出力されること。");

    // 1. メタデータ取得テスト
    let page_count = rasterizer
        .get_page_count_from_file(pdf_path)
        .expect("PDF総ページ数取得に失敗。");
    assert_eq!(page_count, 3, "計 3 ページの物理スライドであること。");

    let dims = rasterizer
        .get_page_dimensions_from_file(pdf_path, 0)
        .expect("寸法取得に失敗。");
    assert!(dims.width_pt > 0.0);
    assert!(dims.height_pt > 0.0);

    // 2. マッピングテーブルの構築テスト
    let mapping_table = result
        .mapping_table_with_fallback(&rasterizer, Some(2))
        .expect("マッピングテーブルの生成に失敗。");

    assert_eq!(mapping_table.frame_count(), 2);
    assert_eq!(mapping_table.total_physical_pages(), 3);

    // スライド 0, ステップ 0 -> 物理ページ 0
    let page_s0 = mapping_table.get_physical_page(0, 0).unwrap();
    assert_eq!(page_s0, 0);

    // スライド 1, ステップ 0 -> 物理ページ 1
    let page_s1_step0 = mapping_table.get_physical_page(1, 0).unwrap();
    assert_eq!(page_s1_step0, 1);

    // スライド 1, ステップ 1 -> 物理ページ 2
    let page_s1_step1 = mapping_table.get_physical_page(1, 1).unwrap();
    assert_eq!(page_s1_step1, 2);

    // 3. 各ステップのビットマップをレンダリング
    let options = RenderOptions::new().with_dpi(150);
    let bitmap0 = rasterizer
        .render_page_to_rgba_with_options(pdf_path, page_s0, &options)
        .expect("スライド 0 の描画に失敗。");
    assert_eq!(bitmap0.page_index, 0);
    assert!(bitmap0.width > 0);
    assert!(bitmap0.height > 0);

    let bitmap1 = rasterizer
        .render_page_to_rgba_with_options(pdf_path, page_s1_step1, &options)
        .expect("スライド 1 ステップ 1 の描画に失敗。");
    assert_eq!(bitmap1.page_index, 2);
}
