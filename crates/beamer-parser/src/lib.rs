//! `rowan` を用いた Beamer ドキュメントの構文解析および CST(具象構文木)操作を提供するクレート。
//!
//! コメントやインデント、未知のマクロを欠落させないロスレスなアイランドパースを支援する。

pub mod cursor;
pub mod lexer;
pub mod parser;
pub mod syntax;
pub mod token_source;

pub use cursor::Cursor;
pub use lexer::{Lexer, Token, tokenize};
pub use parser::{BeamerParser, ParseResult};
pub use syntax::{LatexLanguage, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};
pub use token_source::TokenSource;
