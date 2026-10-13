//! `rowan` を用いた Beamer ドキュメントの構文解析および CST（具体構文木）操作を提供するクレート。
//!
//! コメントやインデント、未知のマクロを欠落させないロスレスなアイランドパースを支援する。

pub mod parser;
pub mod syntax;

pub use parser::{BeamerParser, ParseResult};
pub use syntax::{LatexLanguage, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};
