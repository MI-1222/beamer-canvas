//! CST スライド・オーバーレイステップ・PDF 物理ページの双方向マッピングテーブル。
//!
//! `.nav` 解析結果または PDF の `/PageLabels` メタデータからマッピングテーブルを構築し、
//! $O(1)$ での順引きおよび逆引きルックアップを提供する。

use super::nav_parser::{NavFrameEntry, NavParser};
use serde::{Deserialize, Serialize};

/// 1 つのフレーム(スライド)におけるページおよびステップの対応情報。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameMapping {
    /// CST 上のフレームインデックス(0-based)。
    pub frame_index: usize,
    /// 開始物理ページ番号(0-based)。
    pub start_page: usize,
    /// 終了物理ページ番号(0-based)。
    pub end_page: usize,
    /// このフレームが持つオーバーレイの総ステップ数。
    pub step_count: usize,
    /// 各ステップ(0-based）が表示される物理ページ番号（0-based)の対応配列。
    pub step_to_page: Vec<usize>,
}

/// ドキュメント全体のスライド・ステップ・PDF 物理ページの双方向マッピングテーブル。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlideMappingTable {
    /// フレームごとのマッピング情報一覧(frame_index 順)。
    frames: Vec<FrameMapping>,
    /// 物理ページ番号(0-based)から (frame_index, step_index) への逆引きインデックス。
    page_to_frame_and_step: Vec<Option<(usize, usize)>>,
    /// ドキュメント全体の総物理ページ数。
    total_physical_pages: usize,
}

impl SlideMappingTable {
    /// `.nav` のパース結果一覧(`NavFrameEntry`)からマッピングテーブルを構築する。
    pub fn from_nav_entries(entries: &[NavFrameEntry]) -> Self {
        let mut frames = Vec::with_capacity(entries.len());
        let mut max_page = 0;

        for (frame_idx, entry) in entries.iter().enumerate() {
            let start_page = entry.start_page.saturating_sub(1);
            let end_page = entry.end_page.saturating_sub(1);

            let mut step_to_page = Vec::new();
            if entry.steps.is_empty() {
                // ステップエントリが空の場合は start_page..=end_page を連番ステップとして割り当てる。
                for p in start_page..=end_page {
                    step_to_page.push(p);
                }
            } else {
                for s in &entry.steps {
                    let page_0based = s.physical_page.saturating_sub(1);
                    if !step_to_page.contains(&page_0based) {
                        step_to_page.push(page_0based);
                    }
                }
                // `pause`マクロ等により中間の`slideentry`が出力されていない場合、`start_page..=end_page`の全ページを補完する。
                for p in start_page..=end_page {
                    if !step_to_page.contains(&p) {
                        step_to_page.push(p);
                    }
                }
                step_to_page.sort();
            }

            let step_count = step_to_page
                .len()
                .max(end_page.saturating_sub(start_page) + 1)
                .max(1);

            if end_page >= max_page {
                max_page = end_page + 1;
            }

            frames.push(FrameMapping {
                frame_index: frame_idx,
                start_page,
                end_page,
                step_count,
                step_to_page,
            });
        }

        let mut page_to_frame_and_step = vec![None; max_page];
        for frame in &frames {
            for (step_idx, &physical_page) in frame.step_to_page.iter().enumerate() {
                if physical_page < page_to_frame_and_step.len() {
                    page_to_frame_and_step[physical_page] = Some((frame.frame_index, step_idx));
                }
            }
        }

        Self {
            frames,
            page_to_frame_and_step,
            total_physical_pages: max_page,
        }
    }

    /// `.nav` ファイルの全テキストからマッピングテーブルを構築する。
    pub fn from_nav(nav_content: &str) -> Self {
        let entries = NavParser::parse(nav_content);
        Self::from_nav_entries(&entries)
    }

    /// PDF の `/PageLabels`(各物理ページの論理ラベル文字列配列)からマッピングテーブルを復元する。
    ///
    /// `.nav` が生成されなかった場合や外部 PDF を読み込む際のフォールバックとして機能する。
    pub fn from_page_labels(labels: &[Option<String>]) -> Self {
        if labels.is_empty() {
            return Self {
                frames: Vec::new(),
                page_to_frame_and_step: Vec::new(),
                total_physical_pages: 0,
            };
        }

        let total_physical_pages = labels.len();
        let mut frames = Vec::new();
        let mut page_to_frame_and_step = vec![None; total_physical_pages];

        let mut current_frame_idx = 0;
        let mut current_step_idx = 0;
        let mut current_start_page = 0;
        let mut current_step_to_page = vec![0];
        let mut prev_label = labels[0].as_deref();

        page_to_frame_and_step[0] = Some((0, 0));

        for page_idx in 1..total_physical_pages {
            let label = labels[page_idx].as_deref();
            let is_same_frame = match (prev_label, label) {
                (Some(a), Some(b)) => a == b,
                (None, None) => false,
                _ => false,
            };

            if is_same_frame {
                // 同一フレーム内の次ステップ。
                current_step_idx += 1;
                current_step_to_page.push(page_idx);
                page_to_frame_and_step[page_idx] = Some((current_frame_idx, current_step_idx));
            } else {
                // 前のフレームを確定。
                let end_page = page_idx - 1;
                let step_count = current_step_to_page.len();
                frames.push(FrameMapping {
                    frame_index: current_frame_idx,
                    start_page: current_start_page,
                    end_page,
                    step_count,
                    step_to_page: std::mem::take(&mut current_step_to_page),
                });

                // 新しいフレームの開始。
                current_frame_idx += 1;
                current_step_idx = 0;
                current_start_page = page_idx;
                current_step_to_page.push(page_idx);
                page_to_frame_and_step[page_idx] = Some((current_frame_idx, 0));
                prev_label = label;
            }
        }

        // 最終フレームの確定。
        if !current_step_to_page.is_empty() {
            let end_page = total_physical_pages - 1;
            let step_count = current_step_to_page.len();
            frames.push(FrameMapping {
                frame_index: current_frame_idx,
                start_page: current_start_page,
                end_page,
                step_count,
                step_to_page: current_step_to_page,
            });
        }

        Self {
            frames,
            page_to_frame_and_step,
            total_physical_pages,
        }
    }

    /// `.nav` コンテンツと PDF `/PageLabels` を組み合わせ、フォールバックおよび整合性検証を統合して構築する。
    pub fn from_nav_with_fallback(
        nav_content: Option<&str>,
        page_labels: &[Option<String>],
        expected_frames: Option<usize>,
    ) -> Self {
        let nav_entries = nav_content.map(NavParser::parse).unwrap_or_default();

        let table_from_nav = if !nav_entries.is_empty() {
            Some(Self::from_nav_entries(&nav_entries))
        } else {
            None
        };

        match (table_from_nav, expected_frames) {
            (Some(table), Some(expected)) if table.frames.len() == expected => table,
            (Some(table), None) => table,
            (Some(table), Some(expected)) => {
                // フレーム数に不整合がある場合、PageLabels からの復元を試みる。
                let fallback = Self::from_page_labels(page_labels);
                if fallback.frames.len() == expected {
                    fallback
                } else {
                    table
                }
            }
            (None, _) => Self::from_page_labels(page_labels),
        }
    }

    /// ドキュメント内の総フレーム数を取得する。
    #[inline]
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// ドキュメント全体の総物理ページ数を取得する。
    #[inline]
    pub fn total_physical_pages(&self) -> usize {
        self.total_physical_pages
    }

    /// 全フレームのマッピング情報一覧を取得する。
    #[inline]
    pub fn frames(&self) -> &[FrameMapping] {
        &self.frames
    }

    /// 指定したフレームインデックス(0-based)のマッピング情報を取得する。
    pub fn get_frame(&self, frame_index: usize) -> Option<&FrameMapping> {
        self.frames.get(frame_index)
    }

    /// 指定したフレームとステップ(0-based）に対応する PDF 物理ページ番号(0-based)を取得する（正引き)。
    pub fn get_physical_page(&self, frame_index: usize, step_index: usize) -> Option<usize> {
        let frame = self.get_frame(frame_index)?;
        if step_index < frame.step_to_page.len() {
            Some(frame.step_to_page[step_index])
        } else {
            // ステップ数が指定外の場合は最終物理ページを返す。
            Some(frame.end_page)
        }
    }

    /// 指定した PDF 物理ページ番号(0-based)から (frame_index, step_index) を逆引きする。
    pub fn lookup_frame_and_step(&self, physical_page: usize) -> Option<(usize, usize)> {
        self.page_to_frame_and_step
            .get(physical_page)
            .copied()
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slide_mapping_table_from_nav() {
        let nav = r#"
\headcommand {\slideentry {1}{0}{1}{1/1}{}{0}}
\headcommand {\beamer@framepages {1}{1}}
\headcommand {\slideentry {1}{0}{2}{2/4}{}{0}}
\headcommand {\slideentry {1}{0}{3}{3/4}{}{0}}
\headcommand {\slideentry {1}{0}{4}{4/4}{}{0}}
\headcommand {\beamer@framepages {2}{4}}
"#;
        let table = SlideMappingTable::from_nav(nav);
        assert_eq!(table.frame_count(), 2);
        assert_eq!(table.total_physical_pages(), 4);

        // 正引きテスト
        assert_eq!(table.get_physical_page(0, 0), Some(0)); // Frame 0, Step 0 -> Page 0
        assert_eq!(table.get_physical_page(1, 0), Some(1)); // Frame 1, Step 0 -> Page 1
        assert_eq!(table.get_physical_page(1, 1), Some(2)); // Frame 1, Step 1 -> Page 2
        assert_eq!(table.get_physical_page(1, 2), Some(3)); // Frame 1, Step 2 -> Page 3
        assert_eq!(table.get_physical_page(2, 0), None);

        // 逆引きテスト
        assert_eq!(table.lookup_frame_and_step(0), Some((0, 0)));
        assert_eq!(table.lookup_frame_and_step(1), Some((1, 0)));
        assert_eq!(table.lookup_frame_and_step(2), Some((1, 1)));
        assert_eq!(table.lookup_frame_and_step(3), Some((1, 2)));
        assert_eq!(table.lookup_frame_and_step(4), None);
    }

    #[test]
    fn test_slide_mapping_table_from_page_labels() {
        let labels = vec![
            Some("1".to_string()),
            Some("2".to_string()),
            Some("2".to_string()),
            Some("2".to_string()),
            Some("3".to_string()),
        ];
        let table = SlideMappingTable::from_page_labels(&labels);
        assert_eq!(table.frame_count(), 3);
        assert_eq!(table.total_physical_pages(), 5);

        // Frame 0: Page 0 (Step 0)
        assert_eq!(table.get_physical_page(0, 0), Some(0));
        assert_eq!(table.lookup_frame_and_step(0), Some((0, 0)));

        // Frame 1: Pages 1, 2, 3 (Steps 0, 1, 2)
        assert_eq!(table.get_physical_page(1, 0), Some(1));
        assert_eq!(table.get_physical_page(1, 1), Some(2));
        assert_eq!(table.get_physical_page(1, 2), Some(3));
        assert_eq!(table.lookup_frame_and_step(1), Some((1, 0)));
        assert_eq!(table.lookup_frame_and_step(2), Some((1, 1)));
        assert_eq!(table.lookup_frame_and_step(3), Some((1, 2)));

        // Frame 2: Page 4 (Step 0)
        assert_eq!(table.get_physical_page(2, 0), Some(4));
        assert_eq!(table.lookup_frame_and_step(4), Some((2, 0)));
    }
}
