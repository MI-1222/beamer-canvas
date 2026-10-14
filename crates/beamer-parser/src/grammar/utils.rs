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
    let mut closed = false;

    while !ctx.source.is_eof() {
        // `frame` や `document` の環境境界に到達した場合は異常終了を防ぐため処理を中断する。
        if ctx.source.is_at_env_end_boundary("frame")
            || ctx.source.is_at_env_end_boundary("document")
            || ctx.source.is_at_env_begin_boundary("frame")
        {
            break;
        }

        if ctx.source.at(SyntaxKind::LBrace) {
            depth += 1;
            ctx.bump();
        } else if ctx.source.at(SyntaxKind::RBrace) {
            depth -= 1;
            ctx.bump();
            if depth == 0 {
                closed = true;
                break;
            }
        } else {
            ctx.bump();
        }
    }

    if !closed {
        ctx.errors
            .push("波括弧引数の閉じ波括弧が見つからない。".to_string());
        ctx.start_node(SyntaxKind::ErrorNode);
        ctx.finish_node();
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
    let mut closed = false;

    while !ctx.source.is_eof() {
        // 環境境界に到達した場合は異常終了を防ぐため処理を中断する。
        if ctx.source.is_at_env_end_boundary("frame")
            || ctx.source.is_at_env_end_boundary("document")
            || ctx.source.is_at_env_begin_boundary("frame")
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
                    closed = true;
                    break;
                }
            } else {
                ctx.bump();
            }
        } else {
            ctx.bump();
        }
    }

    if !closed {
        ctx.errors
            .push("角括弧オプションの閉じ角括弧が見つからない。".to_string());
        ctx.start_node(SyntaxKind::ErrorNode);
        ctx.finish_node();
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

    let mut closed = false;

    while !ctx.source.is_eof() {
        if ctx.source.at(SyntaxKind::Newline) {
            // 改行を跨ぐオーバーレイ指定は想定しないため中断する。
            break;
        } else if ctx.source.at(SyntaxKind::RAngle) {
            ctx.bump();
            closed = true;
            break;
        } else {
            ctx.bump();
        }
    }

    if !closed {
        ctx.errors
            .push("オーバーレイスペックの閉じ山括弧が見つからない。".to_string());
        ctx.start_node(SyntaxKind::ErrorNode);
        ctx.finish_node();
    }

    ctx.finish_node();
}

/// 未知または構造解析対象外のマクロを、後続引数を含めて `SyntaxKind::OpaqueMacro` ノードとして解析する。
pub fn parse_opaque_macro(ctx: &mut ParserContext<'_, '_>) {
    ctx.start_node(SyntaxKind::OpaqueMacro);
    ctx.bump(); // コマンド本体(`\textbf` や `\custommacro` 等)を消費する。

    // 直後に続く引数(オーバーレイ `<...>`、オプション `[...]`、引数 `{...}`)を消費する。
    loop {
        // 同一行内の水平空白を消費する。
        if ctx.source.at(SyntaxKind::Whitespace) {
            let next_kind = ctx.source.nth_non_trivia(0).map(|t| t.kind);
            if matches!(
                next_kind,
                Some(SyntaxKind::LAngle | SyntaxKind::LBracket | SyntaxKind::LBrace)
            ) {
                ctx.bump();
            } else {
                break;
            }
        }

        match ctx.source.current_kind() {
            Some(SyntaxKind::LAngle) => {
                parse_overlay_spec(ctx);
            }
            Some(SyntaxKind::LBracket) => {
                parse_bracket_options(ctx, SyntaxKind::OptionsList);
            }
            Some(SyntaxKind::LBrace) => {
                parse_argument(ctx, SyntaxKind::Argument);
            }
            _ => break,
        }
    }

    ctx.finish_node();
}

/// インライン数式 `$...$` または別行立て数式 `$$...$$` を `SyntaxKind::OpaqueBlock` ノードとして解析する。
pub fn parse_inline_math(ctx: &mut ParserContext<'_, '_>) {
    let is_double = ctx.source.at(SyntaxKind::DoubleDollar);
    let target_kind = if is_double {
        SyntaxKind::DoubleDollar
    } else {
        SyntaxKind::Dollar
    };

    ctx.start_node(SyntaxKind::OpaqueBlock);
    ctx.bump(); // 開始記号 `$` または `$$` を消費する。

    let mut closed = false;
    let mut consecutive_newlines: usize = 0;

    while !ctx.source.is_eof() {
        // 環境境界や環境タグに到達した場合は数式が未完了とみなし、直ちに処理を中断する。
        if ctx.source.is_at_env_end_boundary("frame")
            || ctx.source.is_at_env_end_boundary("document")
            || ctx.source.is_at_env_begin_boundary("frame")
            || ctx.source.current_env_begin_boundary_name().is_some()
            || ctx.source.current_env_end_boundary_name().is_some()
        {
            break;
        }

        if ctx.source.at(SyntaxKind::Newline) {
            consecutive_newlines += 1;
            // インライン数式 `$` は連続改行による段落区切りを跨げないため、空行で処理を中断する。
            if !is_double && consecutive_newlines >= 2 {
                break;
            }
            ctx.bump();
            continue;
        } else {
            consecutive_newlines = 0;
        }

        if ctx.source.at(target_kind) {
            ctx.bump(); // 閉じ記号を消費する。
            closed = true;
            break;
        } else if ctx.source.at(SyntaxKind::LBrace) {
            // 数式内の中括弧ネストの深さを維持して消費する。
            parse_argument(ctx, SyntaxKind::OpaqueBlock);
        } else {
            ctx.bump();
        }
    }

    if !closed {
        ctx.errors
            .push("数式の閉じ記号が見つからない。".to_string());
        ctx.start_node(SyntaxKind::ErrorNode);
        ctx.finish_node();
    }

    ctx.finish_node();
}
