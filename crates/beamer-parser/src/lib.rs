//! `rowan` を用いたBeamerドキュメントの構文解析および `CST`(具象構文木)操作を提供するクレート。
//!
//! コメントやインデント、未知のマクロを欠落させないロスレスなアイランドパースを支援する。

pub mod cursor;
pub mod edit;
pub mod grammar;
pub mod lexer;
pub mod parser;
pub mod syntax;
pub mod token_source;

pub use cursor::Cursor;
pub use edit::{DocumentEditor, EditError, FrameHandle, SectionHandle};
pub use lexer::{Lexer, Token, tokenize};
pub use parser::{
    BeamerParser, BlockNode, ColumnNode, ColumnsNode, FrameNode, IncludeGraphicsNode, ItemNode,
    ListNode, ParseResult, PauseNode, SectionNode,
};
pub use syntax::{LatexLanguage, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};
pub use token_source::TokenSource;
