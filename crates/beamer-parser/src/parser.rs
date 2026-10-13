//! 構文解析器および GreenNode 構築モジュール。

use crate::syntax::{SyntaxKind, SyntaxNode};
use rowan::GreenNodeBuilder;

/// Beamer ソースコードの構文解析結果を保持する構造体。
#[derive(Debug, Clone)]
pub struct ParseResult {
    /// 構築されたルート構文ノード。
    pub syntax: SyntaxNode,
    /// 構文解析中に検出されたエラーのリスト。
    pub errors: Vec<String>,
}

/// ソースコードから CST（具体構文木）を構築するパーサー構造体。
#[derive(Debug, Default)]
pub struct BeamerParser;

impl BeamerParser {
    /// 新しいパーサーインスタンスを生成する。
    pub fn new() -> Self {
        Self
    }

    /// LaTeX ソース文字列を解析し、ロスレスな構文ツリーを構築する。
    pub fn parse(&self, text: &str) -> ParseResult {
        let mut builder = GreenNodeBuilder::new();

        builder.start_node(SyntaxKind::Root.into());

        if !text.is_empty() {
            // トークン化の基本実装（全体を OpaqueBlock として保持する安全なフォールバック設計）
            builder.token(SyntaxKind::Text.into(), text);
        }

        builder.finish_node();

        let green_node = builder.finish();
        let syntax = SyntaxNode::new_root(green_node);

        ParseResult {
            syntax,
            errors: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_roundtrip() {
        let source = "\\documentclass{beamer}\n\\begin{document}\n\\end{document}";
        let parser = BeamerParser::new();
        let result = parser.parse(source);

        assert_eq!(result.syntax.text().to_string(), source);
    }
}
