//! 構文解析器および `GreenNode` 構築モジュール。

use crate::lexer::tokenize;
use crate::syntax::{SyntaxKind, SyntaxNode};
use crate::token_source::TokenSource;
use rowan::GreenNodeBuilder;

/// Beamer ソースコードの構文解析結果を保持する構造体。
#[derive(Debug, Clone)]
pub struct ParseResult {
    /// 構築されたルート構文ノード。
    pub syntax: SyntaxNode,
    /// 構文解析中に検出されたエラーのリスト。
    pub errors: Vec<String>,
}

impl ParseResult {
    /// ルート構文ノード `SyntaxKind::Root` の参照を取得する。
    #[inline]
    pub fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }

    /// プリアンブル領域ノード `SyntaxKind::Preamble` を探索して取得する。
    pub fn preamble(&self) -> Option<SyntaxNode> {
        self.syntax
            .children()
            .find(|node| node.kind() == SyntaxKind::Preamble)
    }

    /// 本文領域ノード `SyntaxKind::DocumentBody` を探索して取得する。
    pub fn document_body(&self) -> Option<SyntaxNode> {
        self.syntax
            .children()
            .find(|node| node.kind() == SyntaxKind::DocumentBody)
    }
}

/// ソースコードから具象構文木 `CST` を構築するパーサー構造体。
#[derive(Debug, Default, Clone, Copy)]
pub struct BeamerParser;

impl BeamerParser {
    /// 新しい `BeamerParser` インスタンスを生成する。
    pub fn new() -> Self {
        Self
    }

    /// LaTeX Beamer ソース文字列を解析し、ロスレスな構文木を構築する。
    pub fn parse(&self, text: &str) -> ParseResult {
        let tokens = tokenize(text);
        let mut source = TokenSource::new(tokens);
        let mut builder = GreenNodeBuilder::new();

        builder.start_node(SyntaxKind::Root.into());

        if !source.is_eof() {
            let has_document_env = has_env_begin_in_stream(&source, "document");

            if has_document_env {
                // 1. `\begin{document}` 直前までのプリアンブル領域を解析する。
                builder.start_node(SyntaxKind::Preamble.into());
                while !source.is_eof() && !source.is_at_env_begin_boundary("document") {
                    let token = source.bump().expect("EOF ではない。");
                    builder.token(token.kind.into(), token.text);
                }
                builder.finish_node();

                // 2. `\begin{document}` から `\end{document}` までの本文領域を解析する。
                if source.is_at_env_begin_boundary("document") {
                    builder.start_node(SyntaxKind::DocumentBody.into());

                    // `\begin{document}` タグを消費する。
                    let begin_end_idx = source
                        .env_tag_end_index()
                        .expect("環境タグ終了インデックスが存在する。");
                    while source.cursor() <= begin_end_idx {
                        let token = source.bump().expect("タグ範囲内のトークンが存在する。");
                        builder.token(token.kind.into(), token.text);
                    }

                    // `\end{document}` または `EOF` に達するまで本文トークンを消費する。
                    while !source.is_eof() && !source.is_at_env_end_boundary("document") {
                        let token = source.bump().expect("EOF ではない。");
                        builder.token(token.kind.into(), token.text);
                    }

                    // `\end{document}` タグが存在する場合に消費する。
                    if source.is_at_env_end_boundary("document") {
                        let end_idx = source
                            .env_tag_end_index()
                            .expect("環境タグ終了インデックスが存在する。");
                        while source.cursor() <= end_idx {
                            let token = source.bump().expect("タグ範囲内のトークンが存在する。");
                            builder.token(token.kind.into(), token.text);
                        }
                    }

                    builder.finish_node();
                }

                // 3. `\end{document}` 以降の後続トークンを `Root` 直下に消費する。
                while !source.is_eof() {
                    let token = source.bump().expect("EOF ではない。");
                    builder.token(token.kind.into(), token.text);
                }
            } else {
                // `\begin{document}` が存在しない場合は全体を `Preamble` として保持する。
                builder.start_node(SyntaxKind::Preamble.into());
                while !source.is_eof() {
                    let token = source.bump().expect("EOF ではない。");
                    builder.token(token.kind.into(), token.text);
                }
                builder.finish_node();
            }
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

/// トークンストリーム内に指定環境の開始タグが存在するかを走査して判定する。
fn has_env_begin_in_stream(source: &TokenSource<'_>, env_name: &str) -> bool {
    let mut check_source = source.clone();
    while !check_source.is_eof() {
        if check_source.is_at_env_begin(env_name) {
            return true;
        }
        check_source.bump();
    }
    false
}
