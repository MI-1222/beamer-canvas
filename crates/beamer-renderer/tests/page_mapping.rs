//! Beamer 中間ファイル `.nav` の解析およびスライド・ステップ・PDF 物理ページ双方向マッピングの単体・統合テスト。

use beamer_renderer::{NavParser, SlideMappingTable};

#[test]
fn test_nav_parser_comprehensive() {
    let nav_content = r#"
% Sample .nav file
\headcommand {\slideentry {0}{0}{1}{1/1}{}{0}}
\headcommand {\beamer@framepages {1}{1}}
\headcommand {\sectionentry {1}{First Section}{2}{First Section}{0}}
\headcommand {\beamer@sectionpages {1}{1}}
\headcommand {\beamer@subsectionpages {1}{1}}
\headcommand {\slideentry {1}{0}{1}{2/4}{}{0}}
\headcommand {\slideentry {1}{0}{2}{3/4}{}{0}}
\headcommand {\slideentry {1}{0}{3}{4/4}{}{0}}
\headcommand {\beamer@framepages {2}{4}}
\headcommand {\slideentry {1}{0}{4}{5/5}{Title with \textbf{bold} and {nested}}{0}}
\headcommand {\beamer@framepages {5}{5}}
"#;

    let entries = NavParser::parse(nav_content);
    assert_eq!(entries.len(), 3, "3 つのフレームが抽出されること。");

    // Frame 0
    assert_eq!(entries[0].start_page, 1);
    assert_eq!(entries[0].end_page, 1);
    assert_eq!(entries[0].steps.len(), 1);
    assert_eq!(entries[0].steps[0].physical_page, 1);
    assert_eq!(entries[0].steps[0].step, 1);

    // Frame 1
    assert_eq!(entries[1].start_page, 2);
    assert_eq!(entries[1].end_page, 4);
    assert_eq!(entries[1].steps.len(), 3);
    assert_eq!(entries[1].steps[0].step, 1);
    assert_eq!(entries[1].steps[0].physical_page, 2);
    assert_eq!(entries[1].steps[1].step, 2);
    assert_eq!(entries[1].steps[1].physical_page, 3);
    assert_eq!(entries[1].steps[2].step, 3);
    assert_eq!(entries[1].steps[2].physical_page, 4);

    // Frame 2
    assert_eq!(entries[2].start_page, 5);
    assert_eq!(entries[2].end_page, 5);
    assert_eq!(entries[2].steps.len(), 1);
    assert_eq!(entries[2].steps[0].physical_page, 5);
}

#[test]
fn test_slide_mapping_table_lookup_and_boundary() {
    let nav_content = r#"
\headcommand {\slideentry {1}{0}{1}{1/1}{}{0}}
\headcommand {\beamer@framepages {1}{1}}
\headcommand {\slideentry {1}{0}{1}{2/4}{}{0}}
\headcommand {\slideentry {1}{0}{2}{3/4}{}{0}}
\headcommand {\slideentry {1}{0}{3}{4/4}{}{0}}
\headcommand {\beamer@framepages {2}{4}}
\headcommand {\slideentry {1}{0}{1}{5/5}{}{0}}
\headcommand {\beamer@framepages {5}{5}}
"#;

    let table = SlideMappingTable::from_nav(nav_content);
    assert_eq!(table.frame_count(), 3);
    assert_eq!(table.total_physical_pages(), 5);

    // 順引きのテスト
    assert_eq!(table.get_physical_page(0, 0), Some(0)); // スライド 0, ステップ 0 -> 物理ページ 0
    assert_eq!(table.get_physical_page(1, 0), Some(1)); // スライド 1, ステップ 0 -> 物理ページ 1
    assert_eq!(table.get_physical_page(1, 1), Some(2)); // スライド 1, ステップ 1 -> 物理ページ 2
    assert_eq!(table.get_physical_page(1, 2), Some(3)); // スライド 1, ステップ 2 -> 物理ページ 3
    assert_eq!(table.get_physical_page(1, 99), Some(3)); // 範囲外ステップは最終ページにクランプ
    assert_eq!(table.get_physical_page(2, 0), Some(4)); // スライド 2, ステップ 0 -> 物理ページ 4
    assert_eq!(table.get_physical_page(3, 0), None); // 存在しないスライド

    // 逆引きのテスト
    assert_eq!(table.lookup_frame_and_step(0), Some((0, 0)));
    assert_eq!(table.lookup_frame_and_step(1), Some((1, 0)));
    assert_eq!(table.lookup_frame_and_step(2), Some((1, 1)));
    assert_eq!(table.lookup_frame_and_step(3), Some((1, 2)));
    assert_eq!(table.lookup_frame_and_step(4), Some((2, 0)));
    assert_eq!(table.lookup_frame_and_step(5), None); // 範囲外物理ページ
}

#[test]
fn test_slide_mapping_table_from_page_labels_recovery() {
    // 物理ページ 0〜4 のラベル(同一スライドは同じ論理番号)
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

    assert_eq!(table.get_physical_page(0, 0), Some(0));
    assert_eq!(table.get_physical_page(1, 0), Some(1));
    assert_eq!(table.get_physical_page(1, 1), Some(2));
    assert_eq!(table.get_physical_page(1, 2), Some(3));
    assert_eq!(table.get_physical_page(2, 0), Some(4));

    assert_eq!(table.lookup_frame_and_step(0), Some((0, 0)));
    assert_eq!(table.lookup_frame_and_step(1), Some((1, 0)));
    assert_eq!(table.lookup_frame_and_step(2), Some((1, 1)));
    assert_eq!(table.lookup_frame_and_step(3), Some((1, 2)));
    assert_eq!(table.lookup_frame_and_step(4), Some((2, 0)));
}

#[test]
fn test_slide_mapping_table_from_nav_with_fallback() {
    let labels = vec![
        Some("1".to_string()),
        Some("2".to_string()),
        Some("2".to_string()),
    ];

    // .nav が None の場合は page_labels から復元されること。
    let table = SlideMappingTable::from_nav_with_fallback(None, &labels, Some(2));
    assert_eq!(table.frame_count(), 2);
    assert_eq!(table.total_physical_pages(), 3);
    assert_eq!(table.get_physical_page(1, 1), Some(2));

    // .nav のフレーム数と expected_frames が一致する場合は .nav が優先されること。
    let valid_nav = r#"
\headcommand {\slideentry {1}{0}{1}{1/1}{}{0}}
\headcommand {\beamer@framepages {1}{1}}
\headcommand {\slideentry {1}{0}{1}{2/3}{}{0}}
\headcommand {\slideentry {1}{0}{2}{3/3}{}{0}}
\headcommand {\beamer@framepages {2}{3}}
"#;
    let table2 = SlideMappingTable::from_nav_with_fallback(Some(valid_nav), &labels, Some(2));
    assert_eq!(table2.frame_count(), 2);
    assert_eq!(table2.get_physical_page(1, 1), Some(2));
}
