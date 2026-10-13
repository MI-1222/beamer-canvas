//! `\begin{frame}...\end{frame}` および `\frame{...}` の構文解析モジュール。

use super::ParserContext;
use super::utils::{parse_argument, parse_bracket_options, parse_overlay_spec};
use crate::syntax::SyntaxKind;

/// `\begin{frame} ... \end{frame}` 環境をパースする。
pub fn parse_frame_env(ctx: &mut ParserContext<'_, '_>) {
    ctx.start_node(SyntaxKind::Frame);

    // 行頭インデント空白をノード内に含める。
    ctx.eat_whitespace();

    // `\begin{frame}` タグを消費する。
    if let Some(tag_end_idx) = ctx.source.env_tag_end_index() {
        while ctx.source.cursor() <= tag_end_idx {
            ctx.bump();
        }
    } else {
        // タグが不完全な場合は現在のトークンを1つ消費して進める。
        ctx.bump();
    }

    // オーバーレイ、オプション、タイトル、サブタイトルを含むフレームヘッダーをパースする。
    parse_frame_header(ctx);

    // `\end{frame}` またはドキュメント終了までフレーム本文を消費する。
    while !ctx.source.is_eof() && !ctx.source.is_at_env_end_boundary("frame") {
        // 次のフレーム開始またはドキュメント終了に達した場合は、閉じタグ欠落として同期回復する。
        if ctx.source.is_at_env_begin_boundary("frame")
            || ctx.source.is_at_env_end_boundary("document")
        {
            ctx.errors
                .push("`frame` 環境の閉じタグが見つからないまま次のブロックへ到達した。".to_string());
            break;
        }
        ctx.bump();
    }

    // `\end{frame}` タグを消費する。
    if ctx.source.is_at_env_end_boundary("frame") {
        ctx.eat_whitespace();
        if let Some(tag_end_idx) = ctx.source.env_tag_end_index() {
            while ctx.source.cursor() <= tag_end_idx {
                ctx.bump();
            }
        } else {
            ctx.bump();
        }

        // 行末コメントおよび直後の単一改行をノード内に取り込む。
        ctx.consume_trailing_trivia();
    }

    ctx.finish_node();
}

/// フレームヘッダーノード `FrameHeader` をパースする。
///
/// 同一行内に出現するオーバーレイスペック、オプション角括弧、タイトル中括弧を走査する。
pub fn parse_frame_header(ctx: &mut ParserContext<'_, '_>) {
    ctx.start_node(SyntaxKind::FrameHeader);

    let mut has_title = false;
    let mut has_subtitle = false;

    loop {
        match ctx.source.current_kind() {
            Some(SyntaxKind::Whitespace) => {
                ctx.bump();
            }
            Some(SyntaxKind::LAngle) => {
                parse_overlay_spec(ctx);
            }
            Some(SyntaxKind::LBracket) => {
                parse_bracket_options(ctx, SyntaxKind::FrameOptions);
            }
            Some(SyntaxKind::LBrace) => {
                if !has_title {
                    parse_argument(ctx, SyntaxKind::FrameTitle);
                    has_title = true;
                } else if !has_subtitle {
                    parse_argument(ctx, SyntaxKind::FrameSubtitle);
                    has_subtitle = true;
                } else {
                    // 3つ目以降の中括弧はヘッダーではなく本文とみなす。
                    break;
                }
            }
            _ => {
                // 改行、コメント、本文テキスト等に到達した時点でヘッダーを終了する。
                break;
            }
        }
    }

    ctx.finish_node();
}

/// 旧式の `\frame{ ... }` マクロ構文をパースする。
pub fn parse_legacy_frame(ctx: &mut ParserContext<'_, '_>) {
    ctx.start_node(SyntaxKind::Frame);

    // 行頭インデント空白をノード内に含める。
    ctx.eat_whitespace();

    // `\frame` コマンド本体を消費する。
    ctx.bump();

    // コマンド直後のオプションやオーバーレイを走査する。
    let has_header_elements = matches!(
        ctx.source.nth_non_trivia(0).map(|t| t.kind),
        Some(SyntaxKind::LAngle | SyntaxKind::LBracket)
    );

    if has_header_elements {
        ctx.start_node(SyntaxKind::FrameHeader);
        loop {
            match ctx.source.current_kind() {
                Some(SyntaxKind::Whitespace) => {
                    ctx.bump();
                }
                Some(SyntaxKind::LAngle) => {
                    parse_overlay_spec(ctx);
                }
                Some(SyntaxKind::LBracket) => {
                    parse_bracket_options(ctx, SyntaxKind::FrameOptions);
                }
                _ => break,
            }
        }
        ctx.finish_node();
    }

    // 中括弧前の空白を消費する。
    ctx.eat_whitespace();

    // フレーム全体を囲む波括弧 `{...}` を消費する。
    if ctx.source.at(SyntaxKind::LBrace) {
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

        // 行末コメントおよび直後の単一改行をノード内に取り込む。
        ctx.consume_trailing_trivia();
    }

    ctx.finish_node();
}
