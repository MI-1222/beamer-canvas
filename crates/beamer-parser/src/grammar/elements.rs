//! `frame` 内部の各種構文要素（ブロック、段組み、箇条書き、画像など）の構文解析モジュール。

use super::ParserContext;
use super::utils::{parse_argument, parse_bracket_options, parse_overlay_spec};
use crate::syntax::SyntaxKind;

/// `frame` 本文に含まれるすべての内部要素をパースする。
pub fn parse_frame_content(ctx: &mut ParserContext<'_, '_>) {
    parse_elements_until(ctx, &["frame", "document"], false);
}

/// 指定したいずれかの環境の終了タグ、または終了条件に合致するまで要素を順次パースする。
///
/// `stop_envs`: パースを終了すべき親環境名リスト。`["frame", "document"]` などを渡す。
/// `in_list`: `\item` 内部をパース中であるかを示すフラグ。`true` の場合、次の `\item` コマンドでループを抜ける。
pub fn parse_elements_until(ctx: &mut ParserContext<'_, '_>, stop_envs: &[&str], in_list: bool) {
    while !ctx.source.is_eof() {
        // 1. 指定した親環境の終了タグ境界に達した場合はパースを終了する。
        if ctx.source.is_at_env_end_boundary_any(stop_envs) {
            break;
        }

        // 2. リスト要素内において、次の `\item` コマンド境界に達した場合は項目のパースを終了する。
        if in_list && ctx.source.is_at_command_boundary("\\item") {
            break;
        }

        // 3. `\column` 境界の判定。列内部をパース中に次の `\column` へ達した場合は終了する。
        if stop_envs.contains(&"__column_macro__") && ctx.source.is_at_command_boundary("\\column")
        {
            break;
        }

        // 4. `frame` や `document` の開始タグに遭遇した場合は、閉じタグの欠落とみなしてループを抜ける。
        if ctx.source.is_at_env_begin_boundary("frame")
            || ctx.source.is_at_env_begin_boundary("document")
        {
            ctx.errors
                .push("閉じタグが見つからないまま次のブロックへ到達した。".to_string());
            break;
        }

        let prev_cursor = ctx.source.cursor();

        // 5. 環境開始タグを分岐処理する。
        if let Some(env_name) = ctx.source.current_env_begin_boundary_name() {
            match env_name {
                "block" | "exampleblock" | "alertblock" => {
                    parse_block_env(ctx, env_name);
                }
                "columns" => {
                    parse_columns_env(ctx);
                }
                "column" => {
                    parse_column_env(ctx);
                }
                "itemize" => {
                    parse_list_env(ctx, SyntaxKind::Itemize, "itemize");
                }
                "enumerate" => {
                    parse_list_env(ctx, SyntaxKind::Enumerate, "enumerate");
                }
                "tikzpicture" => {
                    parse_opaque_env(ctx, SyntaxKind::TikzPicture, "tikzpicture");
                }
                _ => {
                    // 未知の環境は不透明環境としてカプセル化する。
                    parse_opaque_env(ctx, SyntaxKind::OpaqueEnv, env_name);
                }
            }
        }
        // 6. 単独コマンドを分岐処理する。
        else if ctx.source.is_at_command_boundary("\\column") {
            parse_column_macro(ctx);
        } else if ctx.source.is_at_command_boundary("\\item") {
            // リスト環境直下で `\item` を見つけた場合。
            parse_item(ctx, stop_envs);
        } else if ctx.source.is_at_command_boundary("\\includegraphics") {
            parse_includegraphics_cmd(ctx);
        } else if ctx.source.is_at_command_boundary("\\pause") {
            parse_pause_cmd(ctx);
        } else if ctx.source.at(SyntaxKind::LAngle) {
            // インラインで現れる単独のオーバーレイスペック `<...>`。
            parse_overlay_spec(ctx);
        } else {
            ctx.bump();
        }

        // 各反復でカーソルが最低1トークン前進していることを保証し、無限ループを防ぐ。
        if ctx.source.cursor() == prev_cursor && !ctx.source.is_eof() {
            ctx.bump();
        }
    }
}

/// `block`, `exampleblock`, `alertblock` 環境をパースする。
pub fn parse_block_env(ctx: &mut ParserContext<'_, '_>, env_name: &str) {
    ctx.start_node(SyntaxKind::Block);

    // 行頭のインデント空白をノード内に含める。
    ctx.eat_whitespace();

    // `\begin{...}` タグを消費する。
    consume_env_tag(ctx);

    // オーバーレイ指定、角括弧オプション、タイトル中括弧を走査する。
    loop {
        match ctx.source.current_kind() {
            Some(SyntaxKind::Whitespace) => {
                ctx.bump();
            }
            Some(SyntaxKind::LAngle) => {
                parse_overlay_spec(ctx);
            }
            Some(SyntaxKind::LBracket) => {
                parse_bracket_options(ctx, SyntaxKind::OptionsList);
            }
            Some(SyntaxKind::LBrace) => {
                // ブロックタイトル引数 `{...}` を消費する。
                parse_argument(ctx, SyntaxKind::Argument);
                break;
            }
            _ => break,
        }
    }

    // ブロック本文をパースする。
    parse_elements_until(ctx, &[env_name, "frame", "document"], false);

    // `\end{...}` タグを消費する。
    if ctx.source.is_at_env_end_boundary(env_name) {
        ctx.eat_whitespace();
        consume_env_tag(ctx);
        ctx.consume_trailing_trivia();
    } else {
        ctx.errors
            .push(format!("`{env_name}` 環境の閉じタグが見つからない。"));
    }

    ctx.finish_node();
}

/// `columns` 環境をパースする。
pub fn parse_columns_env(ctx: &mut ParserContext<'_, '_>) {
    ctx.start_node(SyntaxKind::Columns);

    ctx.eat_whitespace();
    consume_env_tag(ctx);

    // 直後の角括弧オプション `[c]` や `[t,onlytextwidth]` などを走査する。
    loop {
        match ctx.source.current_kind() {
            Some(SyntaxKind::Whitespace) => {
                ctx.bump();
            }
            Some(SyntaxKind::LBracket) => {
                parse_bracket_options(ctx, SyntaxKind::OptionsList);
            }
            _ => break,
        }
    }

    // `\end{columns}` まで内部要素をパースする。
    parse_elements_until(ctx, &["columns", "frame", "document"], false);

    if ctx.source.is_at_env_end_boundary("columns") {
        ctx.eat_whitespace();
        consume_env_tag(ctx);
        ctx.consume_trailing_trivia();
    } else {
        ctx.errors
            .push("`columns` 環境の閉じタグが見つからない。".to_string());
    }

    ctx.finish_node();
}

/// `\begin{column}...\end{column}` 環境をパースする。
pub fn parse_column_env(ctx: &mut ParserContext<'_, '_>) {
    ctx.start_node(SyntaxKind::Column);

    ctx.eat_whitespace();
    consume_env_tag(ctx);

    // オーバーレイ、配置オプション `[t]`、幅指定引数 `{0.5\textwidth}` を走査する。
    let mut has_width = false;
    loop {
        match ctx.source.current_kind() {
            Some(SyntaxKind::Whitespace) => {
                ctx.bump();
            }
            Some(SyntaxKind::LAngle) => {
                parse_overlay_spec(ctx);
            }
            Some(SyntaxKind::LBracket) => {
                parse_bracket_options(ctx, SyntaxKind::OptionsList);
            }
            Some(SyntaxKind::LBrace) => {
                parse_argument(ctx, SyntaxKind::Argument);
                has_width = true;
                break;
            }
            _ => break,
        }
    }

    let _ = has_width;

    // 列本文をパースする。
    parse_elements_until(ctx, &["column", "columns", "frame", "document"], false);

    if ctx.source.is_at_env_end_boundary("column") {
        ctx.eat_whitespace();
        consume_env_tag(ctx);
        ctx.consume_trailing_trivia();
    } else {
        ctx.errors
            .push("`column` 環境の閉じタグが見つからない。".to_string());
    }

    ctx.finish_node();
}

/// マクロ形式の単一列 `\column` をパースする。
pub fn parse_column_macro(ctx: &mut ParserContext<'_, '_>) {
    ctx.start_node(SyntaxKind::Column);

    ctx.eat_whitespace();
    ctx.bump(); // `\column` コマンド本体を消費する。

    // オーバーレイ、配置オプション、幅指定引数を走査する。
    loop {
        match ctx.source.current_kind() {
            Some(SyntaxKind::Whitespace) => {
                ctx.bump();
            }
            Some(SyntaxKind::LAngle) => {
                parse_overlay_spec(ctx);
            }
            Some(SyntaxKind::LBracket) => {
                parse_bracket_options(ctx, SyntaxKind::OptionsList);
            }
            Some(SyntaxKind::LBrace) => {
                parse_argument(ctx, SyntaxKind::Argument);
                break;
            }
            _ => break,
        }
    }

    // 次の `\column`、または親環境の終了まで本文をパースする。
    parse_elements_until(
        ctx,
        &["__column_macro__", "columns", "frame", "document"],
        false,
    );

    ctx.finish_node();
}

/// `itemize`, `enumerate` 環境をパースする。
pub fn parse_list_env(ctx: &mut ParserContext<'_, '_>, kind: SyntaxKind, env_name: &str) {
    ctx.start_node(kind);

    ctx.eat_whitespace();
    consume_env_tag(ctx);

    // リストのデフォルトオーバーレイ指定 `<...>` や角括弧オプション `[...]` を走査する。
    loop {
        match ctx.source.current_kind() {
            Some(SyntaxKind::Whitespace) => {
                ctx.bump();
            }
            Some(SyntaxKind::LAngle) => {
                parse_overlay_spec(ctx);
            }
            Some(SyntaxKind::LBracket) => {
                parse_bracket_options(ctx, SyntaxKind::OptionsList);
            }
            _ => break,
        }
    }

    // リスト本文をパースする。内部の `\item` コマンドは `parse_elements_until` 内で適宜ディスパッチされる。
    parse_elements_until(ctx, &[env_name, "frame", "document"], false);

    if ctx.source.is_at_env_end_boundary(env_name) {
        ctx.eat_whitespace();
        consume_env_tag(ctx);
        ctx.consume_trailing_trivia();
    } else {
        ctx.errors
            .push(format!("`{env_name}` 環境の閉じタグが見つからない。"));
    }

    ctx.finish_node();
}

/// 箇条書き項目 `\item` をパースする。
pub fn parse_item(ctx: &mut ParserContext<'_, '_>, parent_stop_envs: &[&str]) {
    ctx.start_node(SyntaxKind::Item);

    ctx.eat_whitespace();
    ctx.bump(); // `\item` コマンド本体を消費する。

    // 直後のオーバーレイ指定 `<...>` やラベル角括弧 `[...]` を走査する。
    loop {
        match ctx.source.current_kind() {
            Some(SyntaxKind::Whitespace) => {
                ctx.bump();
            }
            Some(SyntaxKind::LAngle) => {
                parse_overlay_spec(ctx);
            }
            Some(SyntaxKind::LBracket) => {
                parse_bracket_options(ctx, SyntaxKind::OptionsList);
            }
            _ => break,
        }
    }

    // 次の `\item`、親リスト環境終了、またはフレーム終了まで本文を貪欲にパースする。
    parse_elements_until(ctx, parent_stop_envs, true);

    ctx.finish_node();
}

/// 画像読み込みコマンド `\includegraphics` をパースする。
pub fn parse_includegraphics_cmd(ctx: &mut ParserContext<'_, '_>) {
    ctx.start_node(SyntaxKind::IncludeGraphics);

    ctx.eat_whitespace();
    ctx.bump(); // `\includegraphics` コマンド本体を消費する。

    loop {
        match ctx.source.current_kind() {
            Some(SyntaxKind::Whitespace) => {
                ctx.bump();
            }
            Some(SyntaxKind::LAngle) => {
                parse_overlay_spec(ctx);
            }
            Some(SyntaxKind::LBracket) => {
                parse_bracket_options(ctx, SyntaxKind::OptionsList);
            }
            Some(SyntaxKind::LBrace) => {
                // 画像ファイルパスの中括弧引数を消費する。
                parse_argument(ctx, SyntaxKind::Argument);
                break;
            }
            _ => break,
        }
    }

    ctx.finish_node();
}

/// 一時停止コマンド `\pause` をパースする。
pub fn parse_pause_cmd(ctx: &mut ParserContext<'_, '_>) {
    ctx.start_node(SyntaxKind::Pause);

    ctx.eat_whitespace();
    ctx.bump(); // `\pause` コマンド本体を消費する。

    loop {
        match ctx.source.current_kind() {
            Some(SyntaxKind::Whitespace) => {
                ctx.bump();
            }
            Some(SyntaxKind::LAngle) => {
                parse_overlay_spec(ctx);
            }
            Some(SyntaxKind::LBracket) => {
                parse_bracket_options(ctx, SyntaxKind::OptionsList);
            }
            _ => break,
        }
    }

    ctx.finish_node();
}

/// `tikzpicture` や数式、未知の環境を不透明環境としてパースする。
pub fn parse_opaque_env(ctx: &mut ParserContext<'_, '_>, kind: SyntaxKind, env_name: &str) {
    ctx.start_node(kind);

    ctx.eat_whitespace();
    consume_env_tag(ctx);

    // 対応する `\end{env_name}` またはフレーム終了までを平坦に消費する。
    while !ctx.source.is_eof() && !ctx.source.is_at_env_end_boundary(env_name) {
        if ctx.source.is_at_env_begin_boundary("frame")
            || ctx.source.is_at_env_end_boundary("frame")
            || ctx.source.is_at_env_end_boundary("document")
        {
            ctx.errors
                .push(format!("`{env_name}` 環境の閉じタグが見つからない。"));
            break;
        }
        ctx.bump();
    }

    if ctx.source.is_at_env_end_boundary(env_name) {
        ctx.eat_whitespace();
        consume_env_tag(ctx);
        ctx.consume_trailing_trivia();
    }

    ctx.finish_node();
}

/// 環境タグ `\begin{...}` または `\end{...}` を消費する。
fn consume_env_tag(ctx: &mut ParserContext<'_, '_>) {
    if let Some(tag_end_idx) = ctx.source.env_tag_end_index() {
        while ctx.source.cursor() <= tag_end_idx {
            ctx.bump();
        }
    } else {
        ctx.bump();
    }
}
