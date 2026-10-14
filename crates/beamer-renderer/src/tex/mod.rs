//! `TeX` コマンドの実行、プロセス管理、環境検出を行うモジュール。

pub mod detector;
pub mod runner;

pub use detector::{
    TexCommandKind, TexEnvironment, TexToolInfo, detect_tool, find_executable_in_path,
    parse_version_output,
};
pub use runner::{CompileOutput, LogMessage, LogStream, TexRunner};
