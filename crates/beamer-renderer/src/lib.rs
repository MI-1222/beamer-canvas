//! `pdfium-render` およびローカル TeX コマンドの非同期実行を管理するクレート。
//!
//! PDF のページラスタライズ、TeX コンパイルプロセスの監視、エラーハンドリングを担当する。

pub mod error;
pub mod pdf;
pub mod tex;

pub use error::{RenderError, Result};
pub use pdf::{PageBitmap, PdfRasterizer, RenderDpi};
pub use tex::{CompileOutput, TexRunner};
