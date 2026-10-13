//! 引数や括弧バランスを安全に処理する構文解析ユーティリティ。

use super::ParserContext;
use crate::syntax::SyntaxKind;

/// 波括弧 `{...}` で囲まれた引数ノードをパースする。
///
/// 内部の波括弧ネストを深さカウンタで追跡し、対応する閉じ波括弧までを消費する。
pub fn parse_argument(ctx: &mut ParserContext<'_, '_>, kind: SyntaxKind) {
    if !ctx.source.at(SyntaxKind::LBrace) {
        return;
    }

    ctx.start_node(kind);
    // 開き波括弧 `{` を消費する。
    ctx.bump();

    let mut depth: usize = 1;
    while !ctx.source.is_eof() {
        if ctx.source.at(SyntaxKind::LBrace) {
            depth += 1;
            ctx.bump();
        } else if ctx.source.at(SyntaxKind::RBrace) {
            depth -= 1;
            ctx.bump();
            if depth == 0 {
                break;
            }
        } else {
            ctx.bump();
        }
    }

    ctx.finish_node();
}

/// 角括弧 `[...]` で囲まれたオプション引数ノードをパースする。
///
/// 内部に `[fragile, environment={frame}]` などの波括弧が含まれる場合も考慮し、
/// 波括弧のネスト深さが 0 の状態にある閉じ角括弧でノードを終了する。
pub fn parse_bracket_options(ctx: &mut ParserContext<'_, '_>, kind: SyntaxKind) {
    if !ctx.source.at(SyntaxKind::LBracket) {
        return;
    }

    ctx.start_node(kind);
    // 開き角括弧 `[` を消費する。
    ctx.bump();

    let mut bracket_depth: usize = 1;
    let mut brace_depth: usize = 0;

    while !ctx.source.is_eof() {
        // 環境境界に到達した場合は異常終了を防ぐためにループを抜ける。
        if ctx.source.is_at_env_end_boundary("frame")
            || ctx.source.is_at_env_end_boundary("document")
        {
            break;
        }

        if ctx.source.at(SyntaxKind::LBrace) {
            brace_depth += 1;
            ctx.bump();
        } else if ctx.source.at(SyntaxKind::RBrace) {
            brace_depth = brace_depth.saturating_sub(1);
            ctx.bump();
        } else if ctx.source.at(SyntaxKind::LBracket) {
            bracket_depth += 1;
            ctx.bump();
        } else if ctx.source.at(SyntaxKind::RBracket) {
            if brace_depth == 0 {
                bracket_depth -= 1;
                ctx.bump();
                if bracket_depth == 0 {
                    break;
                }
            } else {
                ctx.bump();
            }
        } else {
            ctx.bump();
        }
    }

    ctx.finish_node();
}

/// 山括弧 `<...>` で囲まれたオーバーレイスペックノードをパースする。
pub fn parse_overlay_spec(ctx: &mut ParserContext<'_, '_>) {
    if !ctx.source.at(SyntaxKind::LAngle) {
        return;
    }

    ctx.start_node(SyntaxKind::OverlaySpec);
    // 開き山括弧 `<` を消費する。
    ctx.bump();

    while !ctx.source.is_eof() {
        if ctx.source.at(SyntaxKind::Newline) {
            // 改行を跨ぐオーバーレイ指定は想定しないため中断する。
            break;
        } else if ctx.source.at(SyntaxKind::RAngle) {
            ctx.bump();
            break;
        } else {
            ctx.bump();
        }
    }

    ctx.finish_node();
}
