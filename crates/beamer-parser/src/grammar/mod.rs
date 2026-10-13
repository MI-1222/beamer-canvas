//! 各文法要素のパースルーチンを管理するモジュール。

pub mod frame;
pub mod section;
pub mod utils;

use crate::lexer::Token;
use crate::syntax::SyntaxKind;
use crate::token_source::TokenSource;
use rowan::GreenNodeBuilder;

/// 構文解析中のストリーム走査および `CST` ノードの構築コンテキスト。
pub struct ParserContext<'a, 'b> {
    /// トークンストリーム。
    pub source: &'b mut TokenSource<'a>,
    /// `CST` ノードビルダー。
    pub builder: &'b mut GreenNodeBuilder<'static>,
    /// パースエラー蓄積ベクタ。
    pub errors: &'b mut Vec<String>,
}

impl<'a, 'b> ParserContext<'a, 'b> {
    /// 新しい `ParserContext` インスタンスを生成する。
    pub fn new(
        source: &'b mut TokenSource<'a>,
        builder: &'b mut GreenNodeBuilder<'static>,
        errors: &'b mut Vec<String>,
    ) -> Self {
        Self {
            source,
            builder,
            errors,
        }
    }

    /// 指定した構文要素の複合ノードを開始する。
    #[inline]
    pub fn start_node(&mut self, kind: SyntaxKind) {
        self.builder.start_node(kind.into());
    }

    /// 現在開いている複合ノードを閉じる。
    #[inline]
    pub fn finish_node(&mut self) {
        self.builder.finish_node();
    }

    /// 現在のトークンを1つ消費してビルダーに流し込む。
    #[inline]
    pub fn bump(&mut self) -> Option<Token<'a>> {
        if let Some(token) = self.source.bump() {
            self.builder.token(token.kind.into(), token.text);
            Some(token)
        } else {
            None
        }
    }

    /// 連続する水平空白トークンをすべて消費する。
    pub fn eat_whitespace(&mut self) {
        while self.source.at(SyntaxKind::Whitespace) {
            self.bump();
        }
    }

    /// 行末のコメントおよび直後の単一改行をノード内に取り込む。
    ///
    /// スライドやセクションの終端 `Trivia` 帰属ルールに従い、末尾の改行を当該ノードに含める。
    pub fn consume_trailing_trivia(&mut self) {
        self.eat_whitespace();
        if self.source.at(SyntaxKind::Comment) {
            self.bump();
        }
        if self.source.at(SyntaxKind::Newline) {
            self.bump();
        }
    }
}
