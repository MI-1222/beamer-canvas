//! ページマッピング正解率(`Mapping Accuracy`)の検証モジュール。
//!
//! 中間ファイル(`.nav`)の解析結果から構築した `SlideMappingTable` と、
//! `PDFium` 経由で取得した `PDF` の物理ページ数および `/PageLabels` を照合し、
//! フレーム・オーバーレイと物理ページの整合性正解率を検証する。

use crate::report::{GateStatus, MappingReport};
use beamer_renderer::{BuildManager, BuildRequest, PdfRasterizer, TexCommandKind, TexEnvironment};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// 利用可能なTeXエンジンを検出する。
async fn detect_engine() -> Option<TexCommandKind> {
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

/// ページマッピング整合性の検証を実行する。
pub async fn verify_page_mapping() -> Option<MappingReport> {
    let engine = detect_engine().await?;
    let rasterizer = PdfRasterizer::new().ok()?;

    let temp_dir = TempDir::new().ok()?;
    let project_root = temp_dir.path().to_path_buf();
    let main_file = PathBuf::from("main.tex");

    // 3フレーム構成:
    // フレーム0: 静的スライド (1ページ)。
    // フレーム1: 箇条書きオーバーレイ (3ステップ = 3ページ)。
    // フレーム2: 静的結論スライド (1ページ)。
    // 合計: 3フレーム / 5物理ページ。
    let test_tex = r#"\documentclass{beamer}
\usetheme{default}
\begin{document}

\begin{frame}{Slide 1: Intro}
  Single page introduction.
\end{frame}

\begin{frame}<1-3>{Slide 2: Dynamic Steps}
  Step 1 visible immediately.
  \pause
  Step 2 visible next.
  \pause
  Step 3 conclusion.
\end{frame}

\begin{frame}{Slide 3: Conclusion}
  Final single page slide.
\end{frame}

\end{document}
"#;

    let _ = fs::write(project_root.join(&main_file), test_tex);
    let manager = BuildManager::new();

    let mut request = BuildRequest::full(&project_root, &main_file);
    request.engine = Some(engine);

    let build_res = manager.request_build(request).await.ok()?;
    if !build_res.success || build_res.pdf_path.is_none() {
        return None;
    }

    let pdf_path = build_res.pdf_path.as_ref().unwrap();
    let total_pages = rasterizer.get_page_count_from_file(pdf_path).ok()?;

    let mapping_table = build_res
        .mapping_table_with_fallback(&rasterizer, Some(3))?;

    let total_frames = mapping_table.frame_count();
    let mut matched_frames = 0;

    // フレーム0(0始まり): 物理ページ0。
    if mapping_table.get_physical_page(0, 0) == Some(0)
        && mapping_table.lookup_frame_and_step(0) == Some((0, 0))
    {
        matched_frames += 1;
    }

    // フレーム1(0始まり): ステップ0->ページ1, ステップ1->ページ2, ステップ2->ページ3。
    if mapping_table.get_physical_page(1, 0) == Some(1)
        && mapping_table.get_physical_page(1, 1) == Some(2)
        && mapping_table.get_physical_page(1, 2) == Some(3)
        && mapping_table.lookup_frame_and_step(1) == Some((1, 0))
        && mapping_table.lookup_frame_and_step(2) == Some((1, 1))
        && mapping_table.lookup_frame_and_step(3) == Some((1, 2))
    {
        matched_frames += 1;
    }

    // フレーム2(0始まり): 物理ページ4。
    if mapping_table.get_physical_page(2, 0) == Some(4)
        && mapping_table.lookup_frame_and_step(4) == Some((2, 0))
    {
        matched_frames += 1;
    }

    let accuracy_rate = if total_frames > 0 {
        (matched_frames as f64 / total_frames as f64) * 100.0
    } else {
        0.0
    };

    let status = if accuracy_rate >= 100.0 && total_pages == 5 {
        GateStatus::Pass
    } else if accuracy_rate >= 98.0 {
        GateStatus::Warning
    } else {
        GateStatus::Fail
    };

    Some(MappingReport {
        total_frames,
        total_pages,
        matched_frames,
        accuracy_rate,
        status,
    })
}
