//! `pdfium-render` を用いた PDF ページのラスタライズおよび画像変換モジュール。

use crate::error::{RenderError, Result};
use pdfium_render::prelude::*;
use std::path::Path;

/// レンダリング解像度の DPI 設定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderDpi(pub u16);

impl Default for RenderDpi {
    fn default() -> Self {
        Self(150)
    }
}

/// PDF ページのラスタライズ画像データ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageBitmap {
    /// ページインデックス（0開始）。
    pub page_index: usize,
    /// 画像の幅（ピクセル）。
    pub width: u32,
    /// 画像の高さ（ピクセル）。
    pub height: u32,
    /// RGBA バイト列。
    pub rgba_data: Vec<u8>,
}

/// PDF ドキュメントのラスタライザ。
pub struct PdfRasterizer {
    pdfium: Pdfium,
}

impl PdfRasterizer {
    /// システムのダイナミックライブラリから PDFium をバインドして初期化する。
    pub fn new() -> Result<Self> {
        let pdfium = Pdfium::default();
        Ok(Self { pdfium })
    }

    /// PDF ファイルを開いて指定ページのビットマップを生成する。
    pub fn render_page_to_rgba(
        &self,
        pdf_path: &Path,
        page_index: usize,
        dpi: RenderDpi,
    ) -> Result<PageBitmap> {
        let path_str = pdf_path.to_str().ok_or_else(|| RenderError::InvalidPath {
            path: pdf_path.display().to_string(),
        })?;

        let document = self
            .pdfium
            .load_pdf_from_file(path_str, None)
            .map_err(|e| RenderError::Pdfium {
                message: e.to_string(),
            })?;

        let pages = document.pages();
        let page = pages
            .get(page_index as i32)
            .map_err(|e| RenderError::Pdfium {
                message: e.to_string(),
            })?;

        // 72 DPI基準のPDFポイントから指定DPIへのスケーリング比率を計算する。
        let scale = dpi.0 as f32 / 72.0;
        let render_config = PdfRenderConfig::new().scale_page_by_factor(scale);
        let bitmap = page
            .render_with_config(&render_config)
            .map_err(|e| RenderError::Pdfium {
                message: e.to_string(),
            })?;

        let width = bitmap.width() as u32;
        let height = bitmap.height() as u32;
        let rgba_data = bitmap.as_raw_bytes().to_vec();

        Ok(PageBitmap {
            page_index,
            width,
            height,
            rgba_data,
        })
    }
}
