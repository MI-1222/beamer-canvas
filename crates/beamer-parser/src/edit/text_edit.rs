//! `TextRange` に基づくテキスト置換および差分パッチを扱うモジュール。

use rowan::{TextRange, TextSize};

/// テキストに対する単一の置換・挿入・削除操作を表す構造体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    /// 置換対象のバイト範囲。
    pub range: TextRange,
    /// 置換後の文字列。削除時は空文字列とする。
    pub replacement: String,
}

impl TextEdit {
    /// 指定範囲のテキストを新しい文字列で置換する `TextEdit` を生成する。
    pub fn replace(range: TextRange, replacement: impl Into<String>) -> Self {
        Self {
            range,
            replacement: replacement.into(),
        }
    }

    /// 指定オフセット位置に文字列を挿入する `TextEdit` を生成する。
    pub fn insert(offset: TextSize, text: impl Into<String>) -> Self {
        Self {
            range: TextRange::empty(offset),
            replacement: text.into(),
        }
    }

    /// 指定範囲のテキストを削除する `TextEdit` を生成する。
    pub fn delete(range: TextRange) -> Self {
        Self {
            range,
            replacement: String::new(),
        }
    }

    /// 元の文字列に単一の `TextEdit` を適用した新しい文字列を生成する。
    pub fn apply(&self, original: &str) -> String {
        let start = usize::from(self.range.start());
        let end = usize::from(self.range.end());
        let mut result = String::with_capacity(original.len() + self.replacement.len());
        result.push_str(&original[..start]);
        result.push_str(&self.replacement);
        result.push_str(&original[end..]);
        result
    }

    /// 複数の `TextEdit` を元の文字列に適用した新しい文字列を生成する。
    ///
    /// 適用後の位置ずれを防ぐため、後方のオフセットから降順に差分を適用する。
    pub fn apply_all(original: &str, edits: &[TextEdit]) -> String {
        let mut sorted_edits = edits.to_vec();
        // 範囲の開始位置で降順に整列する。開始位置が同一の場合は終了位置で降順とする。
        sorted_edits.sort_by(|a, b| {
            b.range
                .start()
                .cmp(&a.range.start())
                .then_with(|| b.range.end().cmp(&a.range.end()))
        });

        let mut current = original.to_string();
        for edit in sorted_edits {
            current = edit.apply(&current);
        }
        current
    }
}
