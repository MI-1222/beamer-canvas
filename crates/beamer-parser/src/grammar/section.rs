//! `\section` および `\subsection` のセクション境界を解析するモジュール。

use super::ParserContext;
use super::utils::{parse_argument, parse_bracket_options};
use crate::syntax::SyntaxKind;

/// `\section` または `\subsection` ノードをパースする。
pub fn parse_section(ctx: &mut ParserContext<'_, '_>, is_subsection: bool) {
    let kind = if is_subsection {
        SyntaxKind::Subsection
    } else {
        SyntaxKind::Section
    };

    ctx.start_node(kind);

    // 行頭インデント空白をノード内に含める。
    ctx.eat_whitespace();

    // `\section` または `\subsection` コマンド本体を消費する。
    ctx.bump();

    // アスタリスク `*` が続く場合は消費する。
    if let Some(token) = ctx.source.current()
        && token.kind == SyntaxKind::Text
        && token.text == "*"
    {
        ctx.bump();
    }

    // オプション引数前の空白を消費する。
    ctx.eat_whitespace();

    // オプション引数 `[...]` が存在する場合にパースする。
    if ctx.source.at(SyntaxKind::LBracket) {
        parse_bracket_options(ctx, SyntaxKind::OptionsList);
    }

    // タイトル引数前の空白を消費する。
    ctx.eat_whitespace();

    // タイトル引数 `{...}` が存在する場合にパースする。
    if ctx.source.at(SyntaxKind::LBrace) {
        parse_argument(ctx, SyntaxKind::Argument);
    }

    // 行末コメントおよび直後の単一改行をノード内に取り込む。
    ctx.consume_trailing_trivia();

    ctx.finish_node();
}
