//! スライドフレームの並び替え・追加・削除およびインデント調整モジュール。

use crate::edit::DocumentEditor;
use crate::edit::error::EditError;
use crate::edit::handle::FrameHandle;
use crate::edit::text_edit::TextEdit;
use crate::parser::BeamerParser;
use rowan::TextSize;

/// ドキュメント内のスライドに対する移動・追加・削除・インデント調整ロジックを提供する構造体。
pub struct FrameEditor;

impl FrameEditor {
    /// 既存のスライド群から代表的な行頭インデント文字列を検出する。
    ///
    /// スライドが見つからない場合や行頭インデントが存在しない場合は空文字列を返す。
    pub fn detect_indent(frames: &[FrameHandle]) -> String {
        for frame in frames {
            let text = frame.syntax().text().to_string();
            let leading_spaces: String = text
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .collect();
            if !leading_spaces.is_empty() {
                return leading_spaces;
            }
        }
        String::new()
    }

    /// スライドソース文字列に対して指定されたインデントを適用する。
    ///
    /// 内部の相対的なインデント段差を保持したまま、各行のベースインデントを調整する。
    pub fn apply_indent(frame_source: &str, indent: &str) -> String {
        if indent.is_empty() {
            let mut text = frame_source.to_string();
            if !text.ends_with('\n') {
                text.push('\n');
            }
            return text;
        }

        let lines: Vec<&str> = frame_source.lines().collect();
        let base_indent_len = lines
            .first()
            .map(|l| l.chars().take_while(|c| *c == ' ' || *c == '\t').count())
            .unwrap_or(0);

        let mut result = String::new();
        for line in lines {
            if line.trim().is_empty() {
                result.push('\n');
            } else {
                let line_indent_len = line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
                let extra_indent: String = line
                    .chars()
                    .take(line_indent_len)
                    .skip(base_indent_len)
                    .collect();

                result.push_str(indent);
                result.push_str(&extra_indent);
                result.push_str(line.trim_start());
                result.push('\n');
            }
        }
        result
    }

    /// 指定された2つのスライド位置を相互に入れ替える。
    ///
    /// スライド間に存在するセクションやコメントの位置はそのまま保持する。
    pub fn swap_frames(
        editor: &DocumentEditor,
        index_a: usize,
        index_b: usize,
    ) -> Result<DocumentEditor, EditError> {
        let frames = editor.get_frames()?;
        let frame_count = frames.len();

        if index_a >= frame_count {
            return Err(EditError::IndexOutOfBounds {
                index: index_a,
                max: frame_count,
            });
        }
        if index_b >= frame_count {
            return Err(EditError::IndexOutOfBounds {
                index: index_b,
                max: frame_count,
            });
        }

        if index_a == index_b {
            return Ok(editor.clone());
        }

        let first = index_a.min(index_b);
        let second = index_a.max(index_b);

        let first_text = frames[first].syntax().text().to_string();
        let second_text = frames[second].syntax().text().to_string();

        let edits = vec![
            TextEdit::replace(frames[second].syntax().text_range(), first_text),
            TextEdit::replace(frames[first].syntax().text_range(), second_text),
        ];

        let new_text = TextEdit::apply_all(&editor.text(), &edits);
        let parse_result = BeamerParser::new().parse(&new_text);
        Ok(DocumentEditor::new(&parse_result))
    }

    /// 指定されたスライドを別の位置へ移動する。
    ///
    /// スライド間に存在するセクションやコメントの位置はそのまま保持する。
    pub fn move_frame(
        editor: &DocumentEditor,
        from_index: usize,
        to_index: usize,
    ) -> Result<DocumentEditor, EditError> {
        let frames = editor.get_frames()?;
        let frame_count = frames.len();

        if from_index >= frame_count {
            return Err(EditError::IndexOutOfBounds {
                index: from_index,
                max: frame_count,
            });
        }
        if to_index >= frame_count {
            return Err(EditError::IndexOutOfBounds {
                index: to_index,
                max: frame_count,
            });
        }

        if from_index == to_index {
            return Ok(editor.clone());
        }

        let start_slot = from_index.min(to_index);
        let end_slot = from_index.max(to_index);

        // 対象範囲内のスライドテキストを収集し、新しい順序を計算する。
        let mut slot_texts: Vec<String> = (start_slot..=end_slot)
            .map(|i| frames[i].syntax().text().to_string())
            .collect();

        let relative_from = from_index - start_slot;
        let relative_to = to_index - start_slot;

        let moved_text = slot_texts.remove(relative_from);
        slot_texts.insert(relative_to, moved_text);

        // 各スロットのテキストを新しい内容で置換する。
        let mut edits = Vec::new();
        for (i, slot_idx) in (start_slot..=end_slot).enumerate() {
            edits.push(TextEdit::replace(
                frames[slot_idx].syntax().text_range(),
                &slot_texts[i],
            ));
        }

        let new_text = TextEdit::apply_all(&editor.text(), &edits);
        let parse_result = BeamerParser::new().parse(&new_text);
        Ok(DocumentEditor::new(&parse_result))
    }

    /// 指定された通し番号のスライドをドキュメントから削除する。
    pub fn delete_frame(
        editor: &DocumentEditor,
        index: usize,
    ) -> Result<DocumentEditor, EditError> {
        let frames = editor.get_frames()?;
        let frame_count = frames.len();

        if index >= frame_count {
            return Err(EditError::IndexOutOfBounds {
                index,
                max: frame_count,
            });
        }

        let range = frames[index].syntax().text_range();
        let edit = TextEdit::delete(range);
        let mut new_text = edit.apply(&editor.text());

        // `\begin{document}` と `\end{document}` の結合を防ぐため境界改行を補正する。
        if let Some(begin_doc_pos) = new_text.find("\\begin{document}") {
            let after_begin = begin_doc_pos + "\\begin{document}".len();
            if let Some(end_doc_rel) = new_text[after_begin..].find("\\end{document}") {
                let between = &new_text[after_begin..after_begin + end_doc_rel];
                if between.trim().is_empty() && !between.contains('\n') {
                    new_text.insert(after_begin, '\n');
                }
            }
        }

        let parse_result = BeamerParser::new().parse(&new_text);
        Ok(DocumentEditor::new(&parse_result))
    }

    /// 指定された位置に新しいスライドを挿入する。
    ///
    /// `index == frame_count` の場合は末尾に追加する。
    pub fn insert_frame(
        editor: &DocumentEditor,
        index: usize,
        frame_source: &str,
    ) -> Result<DocumentEditor, EditError> {
        let frames = editor.get_frames()?;
        let frame_count = frames.len();

        if index > frame_count {
            return Err(EditError::IndexOutOfBounds {
                index,
                max: frame_count,
            });
        }

        let mut formatted_source = frame_source.to_string();
        if !formatted_source.ends_with('\n') {
            formatted_source.push('\n');
        }

        let offset = if frame_count == 0 {
            // スライドが1枚もない場合は `\begin{document}` の直後に挿入する。
            let body = editor
                .document_body()
                .ok_or(EditError::DocumentBodyNotFound)?;
            let text = editor.text();
            let body_range = body.text_range();
            let body_str = &text[usize::from(body_range.start())..usize::from(body_range.end())];

            if let Some(pos) = body_str.find("\\begin{document}") {
                let mut after_begin = pos + "\\begin{document}".len();
                // 直後に改行文字が存在する場合は改行の直後に挿入する。
                let bytes = body_str.as_bytes();
                if after_begin < bytes.len() && bytes[after_begin] == b'\r' {
                    after_begin += 1;
                }
                if after_begin < bytes.len() && bytes[after_begin] == b'\n' {
                    after_begin += 1;
                }
                body_range.start() + TextSize::from(after_begin as u32)
            } else {
                body_range.start()
            }
        } else if index < frame_count {
            // 既存スライドの直前に挿入する。
            frames[index].syntax().text_range().start()
        } else {
            // 最後のスライドの直後に挿入する。
            frames.last().unwrap().syntax().text_range().end()
        };

        let edit = TextEdit::insert(offset, formatted_source);
        let new_text = edit.apply(&editor.text());
        let parse_result = BeamerParser::new().parse(&new_text);
        Ok(DocumentEditor::new(&parse_result))
    }
}
