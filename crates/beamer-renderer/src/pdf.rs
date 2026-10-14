//! `pdfium-render` を用いた PDF ページのラスタライズおよび画像変換モジュール。
//!
//! PDF ページのビットマップ変換(DPI・ズーム倍率・目標サイズ指定対応)、
//! ページ寸法やラベル情報の事前取得、およびスレッドセーフなラスタライズ処理を提供する。

use crate::error::{RenderError, Result};
use pdfium_render::prelude::*;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

/// レンダリング解像度の DPI 設定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderDpi(pub u16);

impl Default for RenderDpi {
    fn default() -> Self {
        Self(150)
    }
}

/// PDF ページの寸法情報(ポイント単位、72 pt = 1 inch)。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PdfDimensions {
    /// ページの幅(pt)。
    pub width_pt: f32,
    /// ページの高さ(pt)。
    pub height_pt: f32,
}

/// レンダリングパラメータを保持する構造体。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderOptions {
    /// レンダリング解像度(DPI)。
    pub dpi: RenderDpi,
    /// ズーム倍率(1.0 が標準等倍)。
    pub zoom: f32,
    /// 目標描画幅(ピクセル指定、指定時は比率維持またはスケーリング)。
    pub target_width: Option<u32>,
    /// 目標描画高さ(ピクセル指定)。
    pub target_height: Option<u32>,
    /// 白背景(ホワイトバック)を強制するかどうか。
    pub enforce_white_background: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            dpi: RenderDpi::default(),
            zoom: 1.0,
            target_width: None,
            target_height: None,
            enforce_white_background: true,
        }
    }
}

impl RenderOptions {
    /// デフォルト設定で新規作成する。
    pub fn new() -> Self {
        Self::default()
    }

    /// DPI 値を指定して更新する。
    pub fn with_dpi(mut self, dpi: u16) -> Self {
        self.dpi = RenderDpi(dpi);
        self
    }

    /// ズーム倍率を指定して更新する。
    pub fn with_zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom;
        self
    }

    /// 目標幅・高さを指定して自動フィットを設定する。
    pub fn fit_to(mut self, width: u32, height: u32) -> Self {
        self.target_width = Some(width);
        self.target_height = Some(height);
        self
    }

    /// 目標幅のみを指定して設定する。
    pub fn fit_width(mut self, width: u32) -> Self {
        self.target_width = Some(width);
        self
    }

    /// 目標高さのみを指定して設定する。
    pub fn fit_height(mut self, height: u32) -> Self {
        self.target_height = Some(height);
        self
    }

    /// 白背景強制フラグを指定する。
    pub fn with_white_background(mut self, enforce: bool) -> Self {
        self.enforce_white_background = enforce;
        self
    }
}

impl From<RenderDpi> for RenderOptions {
    fn from(dpi: RenderDpi) -> Self {
        Self {
            dpi,
            ..Self::default()
        }
    }
}

/// PDF ページのラスタライズ画像データ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageBitmap {
    /// ページインデックス(0開始)。
    pub page_index: usize,
    /// 画像の幅(ピクセル)。
    pub width: u32,
    /// 画像の高さ(ピクセル)。
    pub height: u32,
    /// RGBA バイト列。
    pub rgba_data: Vec<u8>,
}

/// プラットフォーム共通の標準的な PDFium ダイナミックライブラリの配置場所を探索する。
fn find_pdfium_library_fallback() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;

    let library_name = Pdfium::pdfium_platform_library_name();

    // 1. 環境変数からの探索
    if let Ok(path) = std::env::var("PDFIUM_PATH") {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(dir) = std::env::var("PDFIUM_LIB_DIR") {
        let p = PathBuf::from(dir).join(&library_name);
        if p.exists() {
            return Some(p);
        }
    }

    // 2. 実行ファイル(バイナリ)の隣接ディレクトリからの探索
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(exe_dir) = exe_path.parent()
    {
        let candidate = exe_dir.join(&library_name);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    // 3. カレントディレクトリおよびビルド出力ディレクトリ。
    let local_candidates = [
        PathBuf::from(&library_name),
        PathBuf::from("target/debug").join(&library_name),
        PathBuf::from("target/release").join(&library_name),
        PathBuf::from("../target/debug").join(&library_name),
        PathBuf::from("../target/release").join(&library_name),
        PathBuf::from("../../target/debug").join(&library_name),
        PathBuf::from("../../target/release").join(&library_name),
    ];
    if let Some(found) = local_candidates.into_iter().find(|p| p.exists()) {
        return Some(found);
    }

    // 4. プラットフォーム固有のシステムパスおよびユーザーディレクトリの探索。
    find_platform_specific_pdfium(&library_name)
}

/// `macOS`向けのシステムおよびユーザーライブラリパスを探索する。
#[cfg(target_os = "macos")]
fn find_platform_specific_pdfium(library_name: &std::ffi::OsStr) -> Option<std::path::PathBuf> {
    use std::path::PathBuf;

    // `Homebrew`および標準システムパス。
    let system_candidates = [
        PathBuf::from("/opt/homebrew/lib").join(library_name),
        PathBuf::from("/usr/local/lib").join(library_name),
        PathBuf::from("/usr/lib").join(library_name),
    ];
    if let Some(found) = system_candidates.into_iter().find(|p| p.exists()) {
        return Some(found);
    }

    // `macOS`ユーザーディレクトリ配下の`Python`パッケージ等。
    if let Ok(home) = std::env::var("HOME") {
        let home_path = PathBuf::from(home);
        let user_candidates = [
            home_path.join(".local/lib").join(library_name),
            home_path
                .join("Library/Python/3.13/lib/python/site-packages/pypdfium2_raw")
                .join(library_name),
            home_path
                .join("Library/Python/3.12/lib/python/site-packages/pypdfium2_raw")
                .join(library_name),
            home_path
                .join("Library/Python/3.11/lib/python/site-packages/pypdfium2_raw")
                .join(library_name),
            home_path
                .join("Library/Python/3.10/lib/python/site-packages/pypdfium2_raw")
                .join(library_name),
        ];
        if let Some(found) = user_candidates.into_iter().find(|p| p.exists()) {
            return Some(found);
        }
    }

    None
}

/// `Linux`向けのシステムおよびユーザーライブラリパスを探索する。
#[cfg(target_os = "linux")]
fn find_platform_specific_pdfium(library_name: &std::ffi::OsStr) -> Option<std::path::PathBuf> {
    use std::path::PathBuf;

    // `Linux`標準システムパスおよびマルチアーキテクチャパス。
    let system_candidates = [
        PathBuf::from("/usr/lib").join(library_name),
        PathBuf::from("/usr/local/lib").join(library_name),
        PathBuf::from("/usr/lib/x86_64-linux-gnu").join(library_name),
        PathBuf::from("/usr/lib/aarch64-linux-gnu").join(library_name),
    ];
    if let Some(found) = system_candidates.into_iter().find(|p| p.exists()) {
        return Some(found);
    }

    // `Linux`ユーザーディレクトリ配下。
    if let Ok(home) = std::env::var("HOME") {
        let home_path = PathBuf::from(home);
        let user_candidates = [
            home_path.join(".local/lib").join(library_name),
            home_path
                .join(".local/lib/python3.13/site-packages/pypdfium2_raw")
                .join(library_name),
            home_path
                .join(".local/lib/python3.12/site-packages/pypdfium2_raw")
                .join(library_name),
            home_path
                .join(".local/lib/python3.11/site-packages/pypdfium2_raw")
                .join(library_name),
            home_path
                .join(".local/lib/python3.10/site-packages/pypdfium2_raw")
                .join(library_name),
        ];
        if let Some(found) = user_candidates.into_iter().find(|p| p.exists()) {
            return Some(found);
        }
    }

    None
}

/// `Windows`向けのシステムおよびユーザーライブラリパスを探索する。
#[cfg(target_os = "windows")]
fn find_platform_specific_pdfium(library_name: &std::ffi::OsStr) -> Option<std::path::PathBuf> {
    use std::path::PathBuf;

    // `Windows`システムパス。
    if let Ok(system_root) = std::env::var("SystemRoot") {
        let p = PathBuf::from(system_root)
            .join("System32")
            .join(library_name);
        if p.exists() {
            return Some(p);
        }
    }

    // `Windows`ユーザーディレクトリ配下の`Python`パッケージ等。
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let base = PathBuf::from(local_app_data)
            .join("Programs")
            .join("Python");
        for ver in &["Python313", "Python312", "Python311", "Python310"] {
            let p = base
                .join(ver)
                .join("Lib")
                .join("site-packages")
                .join("pypdfium2_raw")
                .join(library_name);
            if p.exists() {
                return Some(p);
            }
        }
    }

    None
}

/// その他の未対応プラットフォーム向けフォールバック。
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn find_platform_specific_pdfium(_library_name: &std::ffi::OsStr) -> Option<std::path::PathBuf> {
    None
}

/// PDF ドキュメントのスレッドセーフなラスタライザ。
pub struct PdfRasterizer {
    pdfium: Mutex<Pdfium>,
}

impl PdfRasterizer {
    /// システムのダイナミックライブラリまたはフォールバック候補から PDFium をバインドして初期化する。
    pub fn new() -> Result<Self> {
        // すでにバインディングが初期化されているか、システムの標準動的ライブラリ探索を試行。
        match Pdfium::bind_to_system_library() {
            Ok(bindings) => {
                return Ok(Self {
                    pdfium: Mutex::new(Pdfium::new(bindings)),
                });
            }
            Err(PdfiumError::PdfiumLibraryBindingsAlreadyInitialized) => {
                return Ok(Self {
                    pdfium: Mutex::new(Pdfium::default()),
                });
            }
            Err(_) => {}
        }

        // フォールバックパスの探索。
        if let Some(fallback_path) = find_pdfium_library_fallback() {
            match Pdfium::bind_to_library(&fallback_path) {
                Ok(bindings) => {
                    return Ok(Self {
                        pdfium: Mutex::new(Pdfium::new(bindings)),
                    });
                }
                Err(PdfiumError::PdfiumLibraryBindingsAlreadyInitialized) => {
                    return Ok(Self {
                        pdfium: Mutex::new(Pdfium::default()),
                    });
                }
                Err(_) => {}
            }
        }

        // 最終試行としてシステム探索エラーを構造化エラーに変換して返却。
        let bindings = Pdfium::bind_to_system_library().map_err(|e| RenderError::Pdfium {
            message: format!(
                "PDFiumライブラリのロードに失敗しました。環境変数 PDFIUM_PATH またはシステム配置を確認してください: {e}"
            ),
        })?;

        Ok(Self {
            pdfium: Mutex::new(Pdfium::new(bindings)),
        })
    }

    /// 明示的なライブラリファイルパスを指定して PDFium を初期化する。
    pub fn with_library_path(path: impl AsRef<Path>) -> Result<Self> {
        let bindings = match Pdfium::bind_to_library(path.as_ref()) {
            Ok(b) => b,
            Err(PdfiumError::PdfiumLibraryBindingsAlreadyInitialized) => {
                return Ok(Self {
                    pdfium: Mutex::new(Pdfium::default()),
                });
            }
            Err(e) => {
                return Err(RenderError::Pdfium {
                    message: format!("指定パスからのPDFiumロードに失敗しました: {e}"),
                });
            }
        };

        Ok(Self {
            pdfium: Mutex::new(Pdfium::new(bindings)),
        })
    }

    /// PDF ファイルを開いて指定ページのビットマップを生成する。
    ///
    /// ファイルロックを回避するため、内部でバイト列を一括読み込みしてラスタライズする。
    pub fn render_page_to_rgba(
        &self,
        pdf_path: &Path,
        page_index: usize,
        dpi: RenderDpi,
    ) -> Result<PageBitmap> {
        let options = RenderOptions::from(dpi);
        self.render_page_to_rgba_with_options(pdf_path, page_index, &options)
    }

    /// PDF ファイルからオプション指定付きでビットマップを生成する。
    pub fn render_page_to_rgba_with_options(
        &self,
        pdf_path: &Path,
        page_index: usize,
        options: &RenderOptions,
    ) -> Result<PageBitmap> {
        let bytes = fs::read(pdf_path).map_err(|e| RenderError::InvalidPath {
            path: format!("{}: {}", pdf_path.display(), e),
        })?;
        self.render_page_from_bytes(&bytes, page_index, options)
    }

    /// メモリ上の PDF バイト列から指定ページのビットマップを生成する。
    pub fn render_page_from_bytes(
        &self,
        pdf_bytes: &[u8],
        page_index: usize,
        options: &RenderOptions,
    ) -> Result<PageBitmap> {
        let pdfium = self.pdfium.lock().map_err(|e| RenderError::Pdfium {
            message: format!("PDFiumのMutexロック取得に失敗しました: {e}"),
        })?;

        let document = pdfium
            .load_pdf_from_byte_slice(pdf_bytes, None)
            .map_err(|e| RenderError::Pdfium {
                message: e.to_string(),
            })?;

        let pages = document.pages();
        let total_pages = pages.len() as usize;
        if page_index >= total_pages {
            return Err(RenderError::Pdfium {
                message: format!(
                    "指定されたページインデックスが範囲外です(指定: {}, 総ページ数: {})。",
                    page_index, total_pages
                ),
            });
        }

        let page = pages
            .get(page_index as i32)
            .map_err(|e| RenderError::Pdfium {
                message: e.to_string(),
            })?;

        let mut render_config = PdfRenderConfig::new();

        // 拡大縮小比率または目標寸法の適用。
        if let (Some(w), Some(h)) = (options.target_width, options.target_height) {
            render_config = render_config
                .set_target_width(w as i32)
                .set_target_height(h as i32);
        } else if let Some(w) = options.target_width {
            render_config = render_config.set_target_width(w as i32);
        } else if let Some(h) = options.target_height {
            render_config = render_config.set_target_height(h as i32);
        } else {
            // 72 DPI 基準の PDF ポイントから指定 DPI およびズーム倍率へのスケール計算。
            let scale = (options.dpi.0 as f32 / 72.0) * options.zoom;
            render_config = render_config.scale_page_by_factor(scale);
        }

        // 白背景(ホワイトバック)の強制適用。
        if options.enforce_white_background {
            render_config = render_config.set_clear_color(PdfColor::WHITE);
        }

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

    /// メモリ上の PDF バイト列から総ページ数を取得する。
    pub fn get_page_count(&self, pdf_bytes: &[u8]) -> Result<usize> {
        let pdfium = self.pdfium.lock().map_err(|e| RenderError::Pdfium {
            message: format!("PDFiumのMutexロック取得に失敗しました: {e}"),
        })?;

        let document = pdfium
            .load_pdf_from_byte_slice(pdf_bytes, None)
            .map_err(|e| RenderError::Pdfium {
                message: e.to_string(),
            })?;

        Ok(document.pages().len() as usize)
    }

    /// PDF ファイルパスから総ページ数を取得する。
    pub fn get_page_count_from_file(&self, pdf_path: &Path) -> Result<usize> {
        let bytes = fs::read(pdf_path).map_err(|e| RenderError::InvalidPath {
            path: format!("{}: {}", pdf_path.display(), e),
        })?;
        self.get_page_count(&bytes)
    }

    /// メモリ上の PDF バイト列から指定ページの寸法(pt)を取得する。
    pub fn get_page_dimensions(
        &self,
        pdf_bytes: &[u8],
        page_index: usize,
    ) -> Result<PdfDimensions> {
        let pdfium = self.pdfium.lock().map_err(|e| RenderError::Pdfium {
            message: format!("PDFiumのMutexロック取得に失敗しました: {e}"),
        })?;

        let document = pdfium
            .load_pdf_from_byte_slice(pdf_bytes, None)
            .map_err(|e| RenderError::Pdfium {
                message: e.to_string(),
            })?;

        let pages = document.pages();
        let total_pages = pages.len() as usize;
        if page_index >= total_pages {
            return Err(RenderError::Pdfium {
                message: format!(
                    "指定されたページインデックスが範囲外です(指定: {}, 総ページ数: {})。",
                    page_index, total_pages
                ),
            });
        }

        let page = pages
            .get(page_index as i32)
            .map_err(|e| RenderError::Pdfium {
                message: e.to_string(),
            })?;

        Ok(PdfDimensions {
            width_pt: page.width().value,
            height_pt: page.height().value,
        })
    }

    /// PDF ファイルパスから指定ページの寸法(pt)を取得する。
    pub fn get_page_dimensions_from_file(
        &self,
        pdf_path: &Path,
        page_index: usize,
    ) -> Result<PdfDimensions> {
        let bytes = fs::read(pdf_path).map_err(|e| RenderError::InvalidPath {
            path: format!("{}: {}", pdf_path.display(), e),
        })?;
        self.get_page_dimensions(&bytes, page_index)
    }

    /// メモリ上の PDF バイト列から全ページのラベル一覧を取得する。
    pub fn get_page_labels(&self, pdf_bytes: &[u8]) -> Result<Vec<Option<String>>> {
        let pdfium = self.pdfium.lock().map_err(|e| RenderError::Pdfium {
            message: format!("PDFiumのMutexロック取得に失敗しました: {e}"),
        })?;

        let document = pdfium
            .load_pdf_from_byte_slice(pdf_bytes, None)
            .map_err(|e| RenderError::Pdfium {
                message: e.to_string(),
            })?;

        let pages = document.pages();
        let count = pages.len() as usize;
        let mut labels = Vec::with_capacity(count);

        for idx in 0..count {
            let label = match pages.get(idx as i32) {
                Ok(page) => page.label().map(|s| s.to_string()),
                Err(_) => None,
            };
            labels.push(label);
        }

        Ok(labels)
    }

    /// PDF ファイルパスから全ページのラベル一覧を取得する。
    pub fn get_page_labels_from_file(&self, pdf_path: &Path) -> Result<Vec<Option<String>>> {
        let bytes = fs::read(pdf_path).map_err(|e| RenderError::InvalidPath {
            path: format!("{}: {}", pdf_path.display(), e),
        })?;
        self.get_page_labels(&bytes)
    }
}
