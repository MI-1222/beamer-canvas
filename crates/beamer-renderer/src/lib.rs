//! `pdfium-render` によるpdfレンダリングとローカル `TeX` プロセスの実行を管理するクレート。
//!
//! pdfページのラスタライズ、`TeX` コンパイルプロセスの監視、エラー処理、
//! サンドボックス管理、および非同期ビルド調停を担当する。

pub mod error;
pub mod pdf;
pub mod tex;

pub use error::{RenderError, Result};
pub use pdf::{PageBitmap, PdfRasterizer, RenderDpi};
pub use tex::{
    BuildKind, BuildManager, BuildRequest, BuildResult, CompileOutput, Diagnostic,
    DiagnosticSeverity, LogMessage, LogStream, Sandbox, SandboxConfig, TexCommandKind,
    TexEnvironment, TexLogParser, TexLogSummary, TexRunner, TexToolInfo, detect_tool,
};
