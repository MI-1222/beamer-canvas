//! `TeX` コマンドの実行、プロセス管理、環境検出、およびビルド調停を行うモジュール。

pub mod detector;
pub mod log_parser;
pub mod manager;
pub mod runner;
pub mod sandbox;

pub use detector::{
    TexCommandKind, TexEnvironment, TexToolInfo, detect_tool, find_executable_in_path,
    parse_version_output,
};
pub use log_parser::{Diagnostic, DiagnosticSeverity, TexLogParser, TexLogSummary};
pub use manager::{BuildManager, BuildRequest, BuildResult};
pub use runner::{CompileOutput, LogMessage, LogStream, TexRunner};
pub use sandbox::{BuildKind, Sandbox, SandboxConfig};
