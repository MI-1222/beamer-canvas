//! `pdfium-render` による PDF レンダリングとローカル `TeX` プロセスの実行を管理するクレート。
//!
//! PDF ページのラスタライズ、スライドとオーバーレイのページマッピング、
//! `TeX` コンパイルプロセスの監視、エラー処理、サンドボックス管理、および非同期ビルド調停を担当する。

pub mod cache;
pub mod error;
pub mod mapping;
pub mod pdf;
pub mod tex;

pub use cache::{SlideCache, SlideCacheConfig};
pub use error::{RenderError, Result};
pub use mapping::{FrameMapping, NavFrameEntry, NavParser, NavStepEntry, SlideMappingTable};
pub use pdf::{PageBitmap, PdfDimensions, PdfRasterizer, RenderDpi, RenderOptions};
pub use tex::{
    BuildKind, BuildManager, BuildRequest, BuildResult, CompileOutput, Diagnostic,
    DiagnosticSeverity, LogMessage, LogStream, Sandbox, SandboxConfig, TexCommandKind,
    TexEnvironment, TexLogParser, TexLogSummary, TexRunner, TexToolInfo, detect_tool,
};
