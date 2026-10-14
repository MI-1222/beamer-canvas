//! 特定要素のプロパティ置換および `\vspace` 挿入モジュール。

use crate::edit::DocumentEditor;
use crate::edit::error::EditError;
use crate::edit::text_edit::TextEdit;
use crate::parser::{BeamerParser, FrameNode};
use crate::syntax::{SyntaxKind, SyntaxNode};
use rowan::{TextRange, TextSize};

/// スライド内の要素（タイトル、オプション、画像幅、`\vspace` 等）のプロパティ編集ロジックを提供する構造体。
pub struct PropertyEditor;

impl PropertyEditor {
    /// 指定されたスライドのタイトルを更新する。
    ///
    /// ヘッダー引数 `{...}`、または本文内の `\frametitle{...}` を置換する。
    /// タイトルが存在しない場合は適切な位置に新しくタイトルを挿入する。
    pub fn set_frame_title(
        editor: &DocumentEditor,
        frame_index: usize,
        new_title: &str,
    ) -> Result<DocumentEditor, EditError> {
        let frame = editor.get_frame(frame_index)?;
        let frame_syntax = frame.syntax();
        let formatted_title = format!("{{{}}}", new_title);

        // 1. ヘッダー内に `FrameTitle` が存在する場合はその範囲を置換する。
        if let Some(title_node) = frame.node().title_node() {
            let edit = TextEdit::replace(title_node.text_range(), formatted_title);
            let new_text = edit.apply(&editor.text());
            let result = BeamerParser::new().parse(&new_text);
            return Ok(DocumentEditor::new(&result));
        }

        // 2. フレーム本文内に `\frametitle{...}` が存在する場合はその引数を置換する。
        if let Some(arg_range) = find_frametitle_arg_range(frame_syntax) {
            let edit = TextEdit::replace(arg_range, formatted_title);
            let new_text = edit.apply(&editor.text());
            let result = BeamerParser::new().parse(&new_text);
            return Ok(DocumentEditor::new(&result));
        }

        // 3. タイトルが存在しない場合は、適切なヘッダー位置へ挿入する。
        let insert_offset = find_title_insert_offset(frame_syntax)?;
        let edit = TextEdit::insert(insert_offset, formatted_title);
        let new_text = edit.apply(&editor.text());
        let result = BeamerParser::new().parse(&new_text);
        Ok(DocumentEditor::new(&result))
    }

    /// 指定されたスライドのサブタイトルを更新する。
    pub fn set_frame_subtitle(
        editor: &DocumentEditor,
        frame_index: usize,
        new_subtitle: &str,
    ) -> Result<DocumentEditor, EditError> {
        let frame = editor.get_frame(frame_index)?;
        let frame_syntax = frame.syntax();

        // 1. 既存の `FrameSubtitle` がある場合。
        if let Some(subtitle_node) = frame.node().subtitle_node() {
            let range = subtitle_node.text_range();
            let edit = if new_subtitle.trim().is_empty() {
                TextEdit::delete(range)
            } else {
                TextEdit::replace(range, format!("{{{}}}", new_subtitle))
            };
            let new_text = edit.apply(&editor.text());
            let result = BeamerParser::new().parse(&new_text);
            return Ok(DocumentEditor::new(&result));
        }

        // 2. 既存の `FrameSubtitle` がない場合。
        if new_subtitle.trim().is_empty() {
            return Ok(editor.clone());
        }

        let formatted = format!("{{{}}}", new_subtitle);
        if let Some(title_node) = frame.node().title_node() {
            // タイトルの直後に挿入する。
            let offset = title_node.text_range().end();
            let edit = TextEdit::insert(offset, formatted);
            let new_text = edit.apply(&editor.text());
            let result = BeamerParser::new().parse(&new_text);
            Ok(DocumentEditor::new(&result))
        } else {
            // タイトルもない場合は空タイトル `{}` と共に挿入する。
            let offset = find_title_insert_offset(frame_syntax)?;
            let text = format!("{{}}{}", formatted);
            let edit = TextEdit::insert(offset, text);
            let new_text = edit.apply(&editor.text());
            let result = BeamerParser::new().parse(&new_text);
            Ok(DocumentEditor::new(&result))
        }
    }

    /// 指定されたスライドのオプション引数 `[...]` を更新する。
    ///
    /// 空文字列を渡した場合は既存のオプションを削除する。
    pub fn set_frame_options(
        editor: &DocumentEditor,
        frame_index: usize,
        new_options: &str,
    ) -> Result<DocumentEditor, EditError> {
        let frame = editor.get_frame(frame_index)?;
        let frame_syntax = frame.syntax();
        let trimmed = new_options.trim();
        let normalized = trim_brackets(trimmed);

        // 1. 既存の `FrameOptions` が存在する場合。
        if let Some(options_node) = frame.node().options() {
            let range = options_node.text_range();
            let edit = if normalized.is_empty() {
                TextEdit::delete(range)
            } else {
                TextEdit::replace(range, format!("[{}]", normalized))
            };
            let new_text = edit.apply(&editor.text());
            let result = BeamerParser::new().parse(&new_text);
            return Ok(DocumentEditor::new(&result));
        }

        // 2. 既存の `FrameOptions` がない場合。
        if normalized.is_empty() {
            return Ok(editor.clone());
        }

        let insert_offset = find_options_insert_offset(frame_syntax)?;
        let edit = TextEdit::insert(insert_offset, format!("[{}]", normalized));
        let new_text = edit.apply(&editor.text());
        let result = BeamerParser::new().parse(&new_text);
        Ok(DocumentEditor::new(&result))
    }

    /// 指定スライド内の画像要素 `\includegraphics` のオプション文字列全体を更新する。
    pub fn set_image_options(
        editor: &DocumentEditor,
        frame_index: usize,
        image_index: usize,
        new_options: &str,
    ) -> Result<DocumentEditor, EditError> {
        let frame = editor.get_frame(frame_index)?;
        let images = frame.images();

        if image_index >= images.len() {
            return Err(EditError::IndexOutOfBounds {
                index: image_index,
                max: images.len(),
            });
        }

        let target_image = &images[image_index];
        let image_syntax = target_image.syntax();
        let trimmed = new_options.trim();
        let normalized = trim_brackets(trimmed);

        // 既存の `OptionsList` を探す。
        let opt_node = image_syntax
            .children()
            .find(|n| n.kind() == SyntaxKind::OptionsList);

        let edit = if let Some(node) = opt_node {
            if normalized.is_empty() {
                TextEdit::delete(node.text_range())
            } else {
                TextEdit::replace(node.text_range(), format!("[{}]", normalized))
            }
        } else {
            if normalized.is_empty() {
                return Ok(editor.clone());
            }
            // `\includegraphics` コマンドトークンの直後（オーバーレイがあればその後）に挿入する。
            let offset = find_includegraphics_options_insert_offset(image_syntax)?;
            TextEdit::insert(offset, format!("[{}]", normalized))
        };

        let new_text = edit.apply(&editor.text());
        let result = BeamerParser::new().parse(&new_text);
        Ok(DocumentEditor::new(&result))
    }

    /// 指定スライド内の画像要素 `\includegraphics` の `width` オプションのみを更新する。
    ///
    /// 他のオプション（例: `height`, `keepaspectratio` 等）を維持したまま `width` を書き換える。
    pub fn set_image_width(
        editor: &DocumentEditor,
        frame_index: usize,
        image_index: usize,
        new_width: &str,
    ) -> Result<DocumentEditor, EditError> {
        let frame = editor.get_frame(frame_index)?;
        let images = frame.images();

        if image_index >= images.len() {
            return Err(EditError::IndexOutOfBounds {
                index: image_index,
                max: images.len(),
            });
        }

        let target_image = &images[image_index];
        let current_options = target_image.options_text().unwrap_or_default();
        let updated_options = update_kv_in_options(&current_options, "width", new_width.trim());

        Self::set_image_options(editor, frame_index, image_index, &updated_options)
    }

    /// 指定スライド内の画像要素 `\includegraphics` の画像ファイルパス引数を更新する。
    pub fn set_image_path(
        editor: &DocumentEditor,
        frame_index: usize,
        image_index: usize,
        new_path: &str,
    ) -> Result<DocumentEditor, EditError> {
        let frame = editor.get_frame(frame_index)?;
        let images = frame.images();

        if image_index >= images.len() {
            return Err(EditError::IndexOutOfBounds {
                index: image_index,
                max: images.len(),
            });
        }

        let target_image = &images[image_index];
        let arg_node = target_image
            .syntax()
            .children()
            .find(|n| n.kind() == SyntaxKind::Argument)
            .ok_or_else(|| EditError::ElementNotFound {
                message: "画像パス引数ノードが見つからない。".to_string(),
            })?;

        let edit = TextEdit::replace(arg_node.text_range(), format!("{{{}}}", new_path.trim()));
        let new_text = edit.apply(&editor.text());
        let result = BeamerParser::new().parse(&new_text);
        Ok(DocumentEditor::new(&result))
    }

    /// 指定されたスライド内の指定要素の直前または直後に `\vspace` を挿入する。
    pub fn insert_vspace_around_node(
        editor: &DocumentEditor,
        target_node: &SyntaxNode,
        length: &str,
        after: bool,
    ) -> Result<DocumentEditor, EditError> {
        let indent = detect_node_indent(target_node);
        let vspace_str = format!("{}\\vspace{{{}}}\n", indent, length.trim());

        let offset = if after {
            target_node.text_range().end()
        } else {
            target_node.text_range().start()
        };

        let edit = TextEdit::insert(offset, vspace_str);
        let new_text = edit.apply(&editor.text());
        let result = BeamerParser::new().parse(&new_text);
        Ok(DocumentEditor::new(&result))
    }

    /// 指定されたスライドの本文先頭または末尾に `\vspace` を挿入する。
    pub fn insert_frame_vspace(
        editor: &DocumentEditor,
        frame_index: usize,
        length: &str,
        at_top: bool,
    ) -> Result<DocumentEditor, EditError> {
        let frame = editor.get_frame(frame_index)?;
        let frame_syntax = frame.syntax();
        let indent = "  "; // デフォルトインデント。

        let vspace_str = format!("{}\\vspace{{{}}}\n", indent, length.trim());

        let offset = if at_top {
            // ヘッダーの直後に挿入する。
            if let Some(header) = frame.node().header() {
                header.text_range().end()
            } else {
                frame_syntax.text_range().start()
            }
        } else {
            // `\end{frame}` の直前に挿入する。
            find_end_frame_offset(frame_syntax).unwrap_or_else(|| frame_syntax.text_range().end())
        };

        let edit = TextEdit::insert(offset, vspace_str);
        let new_text = edit.apply(&editor.text());
        let result = BeamerParser::new().parse(&new_text);
        Ok(DocumentEditor::new(&result))
    }

    /// 任意の構文ノードのテキスト範囲を新しい文字列で置換する。
    pub fn replace_node(
        editor: &DocumentEditor,
        node: &SyntaxNode,
        new_text: &str,
    ) -> Result<DocumentEditor, EditError> {
        let edit = TextEdit::replace(node.text_range(), new_text);
        let new_source = edit.apply(&editor.text());
        let result = BeamerParser::new().parse(&new_source);
        Ok(DocumentEditor::new(&result))
    }

    /// 任意のバイト範囲を新しい文字列で置換する。
    pub fn replace_range(
        editor: &DocumentEditor,
        range: TextRange,
        new_text: &str,
    ) -> Result<DocumentEditor, EditError> {
        let edit = TextEdit::replace(range, new_text);
        let new_source = edit.apply(&editor.text());
        let result = BeamerParser::new().parse(&new_source);
        Ok(DocumentEditor::new(&result))
    }
}

/// 前後の角括弧 `[` と `]` を除去する。
fn trim_brackets(s: &str) -> &str {
    let trimmed = s.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1].trim()
    } else {
        trimmed
    }
}

/// カンマ区切りのオプション文字列内で特定のキーを更新または追加する。
fn update_kv_in_options(options: &str, key: &str, new_value: &str) -> String {
    let mut parts: Vec<String> = options
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let mut found = false;
    for part in &mut parts {
        let is_target = if let Some(eq_pos) = part.find('=') {
            part[..eq_pos].trim() == key
        } else {
            part.trim() == key
        };

        if is_target {
            *part = format!("{}={}", key, new_value);
            found = true;
            break;
        }
    }

    if !found {
        parts.push(format!("{}={}", key, new_value));
    }

    parts.join(", ")
}

/// フレーム本文直下の `\frametitle{...}` の引数 `{...}` のバイト範囲を取得する。
fn find_frametitle_arg_range(frame_syntax: &SyntaxNode) -> Option<TextRange> {
    let tokens: Vec<_> = frame_syntax
        .descendants_with_tokens()
        .filter_map(|el| el.into_token())
        .collect();

    let mut i = 0;
    while i < tokens.len() {
        if tokens[i].kind() == SyntaxKind::Command && tokens[i].text() == "\\frametitle" {
            i += 1;
            while i < tokens.len() && tokens[i].kind().is_trivia() {
                i += 1;
            }
            if i < tokens.len() && tokens[i].kind() == SyntaxKind::LBrace {
                let start_offset = tokens[i].text_range().start();
                let mut depth = 1;
                i += 1;
                while i < tokens.len() {
                    if tokens[i].kind() == SyntaxKind::LBrace {
                        depth += 1;
                    } else if tokens[i].kind() == SyntaxKind::RBrace {
                        depth -= 1;
                        if depth == 0 {
                            let end_offset = tokens[i].text_range().end();
                            return Some(TextRange::new(start_offset, end_offset));
                        }
                    }
                    i += 1;
                }
            }
        } else {
            i += 1;
        }
    }
    None
}

/// タイトルが存在しないスライドにおいて、新しくタイトルを挿入すべきオフセット位置を計算する。
fn find_title_insert_offset(frame_syntax: &SyntaxNode) -> Result<TextSize, EditError> {
    let frame_node = FrameNode::new(frame_syntax).ok_or_else(|| EditError::ElementNotFound {
        message: "フレームノードではない。".to_string(),
    })?;

    // オプションがあればその末尾。
    if let Some(options) = frame_node.options() {
        return Ok(options.text_range().end());
    }

    // オーバーレイスペックがあればその末尾。
    if let Some(overlay) = frame_node.overlay_spec() {
        return Ok(overlay.text_range().end());
    }

    // `FrameHeader` があればヘッダーの開始位置（\begin{frame} の直後）。
    if let Some(header) = frame_node.header() {
        return Ok(header.text_range().start());
    }

    // `\frame{` マクロ形式の場合。
    for tok in frame_syntax
        .descendants_with_tokens()
        .filter_map(|el| el.into_token())
    {
        if tok.kind() == SyntaxKind::Command && tok.text() == "\\frame" {
            return Ok(tok.text_range().end());
        }
    }

    Ok(frame_syntax.text_range().start())
}

/// オプション `[...]` が存在しないスライドにおいて、新しくオプションを挿入すべきオフセット位置を計算する。
fn find_options_insert_offset(frame_syntax: &SyntaxNode) -> Result<TextSize, EditError> {
    let frame_node = FrameNode::new(frame_syntax).ok_or_else(|| EditError::ElementNotFound {
        message: "フレームノードではない。".to_string(),
    })?;

    // オーバーレイスペックがあればその末尾。
    if let Some(overlay) = frame_node.overlay_spec() {
        return Ok(overlay.text_range().end());
    }

    // `FrameHeader` の開始位置（\begin{frame} の直後）。
    if let Some(header) = frame_node.header() {
        return Ok(header.text_range().start());
    }

    Ok(frame_syntax.text_range().start())
}

/// `\includegraphics` 内でオプション引数を挿入すべきオフセット位置を計算する。
fn find_includegraphics_options_insert_offset(
    image_syntax: &SyntaxNode,
) -> Result<TextSize, EditError> {
    // オーバーレイ `<...>` があればその直後。
    if let Some(overlay) = image_syntax
        .children()
        .find(|n| n.kind() == SyntaxKind::OverlaySpec)
    {
        return Ok(overlay.text_range().end());
    }

    // `\includegraphics` トークンの直後。
    if let Some(cmd_token) = image_syntax
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|tok| tok.kind() == SyntaxKind::Command && tok.text() == "\\includegraphics")
    {
        return Ok(cmd_token.text_range().end());
    }

    Ok(image_syntax.text_range().start())
}

/// ノードの直前の空白トークンから行頭インデント文字列を検出する。
fn detect_node_indent(node: &SyntaxNode) -> String {
    if let Some(prev) = node.prev_sibling_or_token()
        && let Some(token) = prev.into_token()
        && token.kind() == SyntaxKind::Whitespace
    {
        let text = token.text();
        if let Some(last_nl) = text.rfind('\n') {
            return text[last_nl + 1..].to_string();
        }
        return text.to_string();
    }
    String::new()
}

/// スライド内の `\end{frame}` トークンの開始オフセットを探索する。
fn find_end_frame_offset(frame_syntax: &SyntaxNode) -> Option<TextSize> {
    for tok in frame_syntax
        .descendants_with_tokens()
        .filter_map(|el| el.into_token())
    {
        if tok.kind() == SyntaxKind::Command && tok.text() == "\\end" {
            return Some(tok.text_range().start());
        }
    }
    None
}
