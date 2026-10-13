//! pdfium-renderによるPDFレンダリングとローカルTeXプロセスの実行を管理するクレート。
//!
//! PDFページのラスタライズ、TeXコンパイルプロセスの監視、エラー処理を担当する。

pub mod error;
pub mod pdf;
pub mod tex;

pub use error::{RenderError, Result};
pub use pdf::{PageBitmap, PdfRasterizer, RenderDpi};
pub use tex::{CompileOutput, LogMessage, LogStream, TexRunner};
