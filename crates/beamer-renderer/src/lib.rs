//! `pdfium-render` による `PDF` レンダリングとローカル `TeX` プロセスの実行を管理するクレート。
//!
//! `PDF` ページのラスタライズ、`TeX` コンパイルプロセスの監視、エラー処理を担当する。

pub mod error;
pub mod pdf;
pub mod tex;

pub use error::{RenderError, Result};
pub use pdf::{PageBitmap, PdfRasterizer, RenderDpi};
pub use tex::{
    CompileOutput, LogMessage, LogStream, TexCommandKind, TexEnvironment, TexRunner, TexToolInfo,
    detect_tool,
};
