//! `DocumentEditor` およびスライド・セクション抽出APIのテスト。

use beamer_parser::{BeamerParser, EditError, SyntaxKind};

#[test]
fn test_get_frames_and_sections_basic() {
    let source = r#"\documentclass{beamer}
\begin{document}

\begin{frame}{タイトルスライド}
  イントロダクション
\end{frame}

\section{基本概念}
\begin{frame}{概念1}{サブタイトル1}
  スライド1の内容
\end{frame}

\subsection{詳細設計}
\begin{frame}[plain]{設計詳細}
  スライド2の内容
\end{frame}

\section{まとめ}
\begin{frame}
  \frametitle{最終スライド}
  まとめの内容
\end{frame}

\end{document}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);
    let editor = result.editor();

    // スライド総数とセクション総数の確認。
    assert_eq!(editor.frame_count().unwrap(), 4);
    assert_eq!(editor.section_count().unwrap(), 3);

    let frames = editor.get_frames().expect("スライド抽出に失敗。");
    assert_eq!(frames.len(), 4);

    // スライド 0: 所属セクションなしのタイトルスライド。
    assert_eq!(frames[0].index(), 0);
    assert_eq!(frames[0].slide_number(), 1);
    assert_eq!(frames[0].title_text().as_deref(), Some("タイトルスライド"));
    assert_eq!(frames[0].subtitle_text(), None);
    assert_eq!(frames[0].section_index(), None);
    assert_eq!(frames[0].subsection_index(), None);
    assert!(!frames[0].has_errors());

    // スライド 1: セクション 0「基本概念」配下のスライド。
    assert_eq!(frames[1].index(), 1);
    assert_eq!(frames[1].slide_number(), 2);
    assert_eq!(frames[1].title_text().as_deref(), Some("概念1"));
    assert_eq!(frames[1].subtitle_text().as_deref(), Some("サブタイトル1"));
    assert_eq!(frames[1].section_index(), Some(0));
    assert_eq!(frames[1].subsection_index(), None);

    // スライド 2: サブセクション 1「詳細設計」配下のスライド。
    assert_eq!(frames[2].index(), 2);
    assert_eq!(frames[2].slide_number(), 3);
    assert_eq!(frames[2].title_text().as_deref(), Some("設計詳細"));
    assert_eq!(frames[2].options_text().as_deref(), Some("plain"));
    assert_eq!(frames[2].section_index(), Some(0)); // 直近のセクション。
    assert_eq!(frames[2].subsection_index(), Some(1)); // 直近のサブセクション。

    // スライド 3: セクション 2「まとめ」配下のスライド(`\frametitle` によるタイトル)。
    assert_eq!(frames[3].index(), 3);
    assert_eq!(frames[3].slide_number(), 4);
    assert_eq!(frames[3].title_text().as_deref(), Some("最終スライド"));
    assert_eq!(frames[3].section_index(), Some(2));
    assert_eq!(frames[3].subsection_index(), None);

    // セクション一覧の確認。
    let sections = editor.get_sections().expect("セクション抽出に失敗。");
    assert_eq!(sections.len(), 3);

    // セクション 0: `\section{基本概念}`
    assert_eq!(sections[0].index(), 0);
    assert!(!sections[0].is_subsection());
    assert_eq!(sections[0].title_text().as_deref(), Some("基本概念"));
    assert_eq!(sections[0].parent_section_index(), None);
    assert_eq!(sections[0].frame_indices(), &[1, 2]);

    // セクション 1: `\subsection{詳細設計}`
    assert_eq!(sections[1].index(), 1);
    assert!(sections[1].is_subsection());
    assert_eq!(sections[1].title_text().as_deref(), Some("詳細設計"));
    assert_eq!(sections[1].parent_section_index(), Some(0));
    assert_eq!(sections[1].frame_indices(), &[2]);

    // セクション 2: `\section{まとめ}`
    assert_eq!(sections[2].index(), 2);
    assert!(!sections[2].is_subsection());
    assert_eq!(sections[2].title_text().as_deref(), Some("まとめ"));
    assert_eq!(sections[2].parent_section_index(), None);
    assert_eq!(sections[2].frame_indices(), &[3]);
}

#[test]
fn test_syntax_node_ptr_roundtrip() {
    let source = r#"\documentclass{beamer}
\begin{document}
\begin{frame}{ポインタテスト}
  本文
\end{frame}
\end{document}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);
    let frames = result.get_frames().expect("スライド抽出に失敗。");

    let handle = &frames[0];
    let ptr = handle.ptr();

    // `SyntaxNodePtr` を使ってルートノードから同一の構文ノードを復元できることを検証する。
    let resolved_node = ptr.to_node(result.syntax());
    assert_eq!(resolved_node.kind(), SyntaxKind::Frame);
    assert_eq!(
        resolved_node.text().to_string(),
        handle.syntax().text().to_string()
    );
}

#[test]
fn test_get_frame_and_section_by_index() {
    let source = r#"\documentclass{beamer}
\begin{document}
\section{第1章}
\begin{frame}{スライド1}\end{frame}
\begin{frame}{スライド2}\end{frame}
\end{document}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);
    let editor = result.editor();

    let frame0 = editor.get_frame(0).expect("インデックス 0 の取得に失敗。");
    assert_eq!(frame0.title_text().as_deref(), Some("スライド1"));

    let frame1 = editor.get_frame(1).expect("インデックス 1 の取得に失敗。");
    assert_eq!(frame1.title_text().as_deref(), Some("スライド2"));

    let err_frame = editor.get_frame(2).unwrap_err();
    assert_eq!(err_frame, EditError::IndexOutOfBounds { index: 2, max: 2 });

    let sec0 = editor.get_section(0).expect("セクション 0 の取得に失敗。");
    assert_eq!(sec0.title_text().as_deref(), Some("第1章"));

    let err_sec = editor.get_section(1).unwrap_err();
    assert_eq!(err_sec, EditError::IndexOutOfBounds { index: 1, max: 1 });
}

#[test]
fn test_error_handling_when_no_document_body() {
    let source = r#"\documentclass{beamer}
\title{タイトルのみ}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);
    let editor = result.editor();

    assert_eq!(
        editor.get_frames().unwrap_err(),
        EditError::DocumentBodyNotFound
    );
    assert_eq!(
        editor.get_sections().unwrap_err(),
        EditError::DocumentBodyNotFound
    );
}

#[test]
fn test_error_handling_when_slide_has_syntax_error() {
    // スライド 1 は閉じタグが欠落しており `ErrorNode` が含まれる。
    let source = r#"\documentclass{beamer}
\begin{document}

\begin{frame}{正常スライド1}
  本文1
\end{frame}

\begin{frame}{壊れたスライド}
  閉じタグなし

\begin{frame}{正常スライド2}
  本文2
\end{frame}

\end{document}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);
    let editor = result.editor();

    // 1. 厳格モード `get_frames()` はエラーを返す。
    match editor.get_frames() {
        Err(EditError::SyntaxError { index, .. }) => {
            assert_eq!(index, Some(1));
        }
        other => panic!("SyntaxError を期待したが異なる結果: {:?}", other),
    }

    // 2. 寛容モード `get_valid_frames()` は壊れたスライドをスキップし、健全な2枚のみを返す。
    let valid_frames = editor
        .get_valid_frames()
        .expect("健全なスライドのみ取得可能。");
    assert_eq!(valid_frames.len(), 2);
    assert_eq!(
        valid_frames[0].title_text().as_deref(),
        Some("正常スライド1")
    );
    assert_eq!(
        valid_frames[1].title_text().as_deref(),
        Some("正常スライド2")
    );

    // 3. 全取得モード `get_all_frames_unchecked()` は全スライドを取得し、エラーフラグを保持する。
    let all_frames = editor
        .get_all_frames_unchecked()
        .expect("全スライド取得可能。");
    assert_eq!(all_frames.len(), 3);
    assert!(!all_frames[0].has_errors());
    assert!(all_frames[1].has_errors());
    assert!(!all_frames[2].has_errors());
}

#[test]
fn test_legacy_frame_macro_extraction() {
    let source = r#"\documentclass{beamer}
\begin{document}

\frame{
  旧式マクロによるスライド
}

\begin{frame}{通常スライド}
  通常スライドの内容
\end{frame}

\end{document}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);
    let frames = result.get_frames().expect("スライド抽出に失敗。");

    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].index(), 0);
    assert_eq!(frames[1].index(), 1);
    assert_eq!(frames[1].title_text().as_deref(), Some("通常スライド"));
}

#[test]
fn test_swap_frames_reversibility() {
    let source = r#"\documentclass{beamer}
\begin{document}

\begin{frame}{スライドA}
  内容A
\end{frame}

% 中間のコメント
\section{テスト章}

\begin{frame}{スライドB}
  内容B
\end{frame}

\begin{frame}{スライドC}
  内容C
\end{frame}

\end{document}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);
    let editor = result.editor();

    // 1. スライド 0 と スライド 1 の位置を入れ替える。
    let swapped = editor
        .swap_frames(0, 1)
        .expect("スライドの入れ替えに失敗。");
    let swapped_frames = swapped.get_frames().expect("スライド一覧の取得に失敗。");
    assert_eq!(swapped_frames[0].title_text().as_deref(), Some("スライドB"));
    assert_eq!(swapped_frames[1].title_text().as_deref(), Some("スライドA"));
    assert_eq!(swapped_frames[2].title_text().as_deref(), Some("スライドC"));

    // スライド間にあるコメントとセクションの保持を検証する。
    assert!(
        swapped
            .text()
            .contains("% 中間のコメント\n\\section{テスト章}")
    );

    // 2. 再度同一ペアを入れ替えて元の順序へ復元できることを検証する。
    let restored = swapped
        .swap_frames(0, 1)
        .expect("再度のスライド入れ替えに失敗。");
    assert_eq!(restored.text(), source);
}

#[test]
fn test_move_frame_forward_and_backward() {
    let source = r#"\documentclass{beamer}
\begin{document}

\begin{frame}{スライド0}
  内容0
\end{frame}

\begin{frame}{スライド1}
  内容1
\end{frame}

\begin{frame}{スライド2}
  内容2
\end{frame}

\end{document}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);
    let editor = result.editor();

    // 先頭スライドを末尾へ移動する(インデックス 0 から 2)。
    let moved_to_end = editor
        .move_frame(0, 2)
        .expect("末尾へのスライド移動に失敗。");
    let frames_after_end = moved_to_end
        .get_frames()
        .expect("スライド一覧の取得に失敗。");
    assert_eq!(
        frames_after_end[0].title_text().as_deref(),
        Some("スライド1")
    );
    assert_eq!(
        frames_after_end[1].title_text().as_deref(),
        Some("スライド2")
    );
    assert_eq!(
        frames_after_end[2].title_text().as_deref(),
        Some("スライド0")
    );

    // 末尾スライドを先頭へ移動する(インデックス 2 から 0)。
    let moved_to_start = moved_to_end
        .move_frame(2, 0)
        .expect("先頭へのスライド移動に失敗。");
    assert_eq!(moved_to_start.text(), source);
}

#[test]
fn test_delete_frame_boundary_cases() {
    let source = r#"\documentclass{beamer}
\begin{document}

\begin{frame}{スライド0}
  内容0
\end{frame}

\begin{frame}{スライド1}
  内容1
\end{frame}

\begin{frame}{スライド2}
  内容2
\end{frame}

\end{document}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);
    let editor = result.editor();

    // 1. 中間スライド(インデックス 1)を削除する。
    let deleted_middle = editor.delete_frame(1).expect("中間スライドの削除に失敗。");
    assert_eq!(deleted_middle.frame_count().unwrap(), 2);
    let frames = deleted_middle.get_frames().unwrap();
    assert_eq!(frames[0].title_text().as_deref(), Some("スライド0"));
    assert_eq!(frames[1].title_text().as_deref(), Some("スライド2"));

    // 2. 先頭スライド(インデックス 0)を削除する。
    let deleted_first = deleted_middle
        .delete_frame(0)
        .expect("先頭スライドの削除に失敗。");
    assert_eq!(deleted_first.frame_count().unwrap(), 1);
    let frames = deleted_first.get_frames().unwrap();
    assert_eq!(frames[0].title_text().as_deref(), Some("スライド2"));

    // 3. 最後のスライドを削除して全スライドを除去する。
    let deleted_all = deleted_first
        .delete_frame(0)
        .expect("全スライドの削除に失敗。");
    assert_eq!(deleted_all.frame_count().unwrap(), 0);
    // `\begin{document}` と `\end{document}` が維持されていることを検証する。
    let parse_res = deleted_all.to_parse_result();
    assert!(!parse_res.has_errors());
    assert!(deleted_all.document_body().is_some());
}

#[test]
fn test_insert_frame_and_auto_indent() {
    let source = r#"\documentclass{beamer}
\begin{document}
  \begin{frame}{既存スライド1}
    本文1
  \end{frame}
  \begin{frame}{既存スライド2}
    本文2
  \end{frame}
\end{document}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);
    let editor = result.editor();

    // 2文字の半角空白がインデントとして検出されることを検証する。
    let indent = editor.detect_indent();
    assert_eq!(indent, "  ");

    // 中間位置(インデックス 1)へ自動インデントを適用してスライドを挿入する。
    let new_slide_snippet = r#"\begin{frame}{新規スライド}
  新規本文
\end{frame}
"#;
    let with_inserted = editor
        .insert_frame_with_auto_indent(1, new_slide_snippet)
        .expect("スライドの挿入に失敗。");

    assert_eq!(with_inserted.frame_count().unwrap(), 3);
    let frames = with_inserted.get_frames().unwrap();
    assert_eq!(frames[0].title_text().as_deref(), Some("既存スライド1"));
    assert_eq!(frames[1].title_text().as_deref(), Some("新規スライド"));
    assert_eq!(frames[2].title_text().as_deref(), Some("既存スライド2"));

    // 挿入したスライドの行頭インデントが半角空白2文字であることを検証する。
    let inserted_text = frames[1].syntax().text().to_string();
    assert!(inserted_text.starts_with("  \\begin{frame}"));

    // 末尾へのスライド追加を検証する。
    let with_pushed = with_inserted
        .push_frame_with_auto_indent(r#"\begin{frame}{末尾スライド}\end{frame}"#)
        .expect("末尾へのスライド追加に失敗。");
    assert_eq!(with_pushed.frame_count().unwrap(), 4);
    let frames = with_pushed.get_frames().unwrap();
    assert_eq!(frames[3].title_text().as_deref(), Some("末尾スライド"));
}

#[test]
fn test_insert_into_empty_document() {
    let source = r#"\documentclass{beamer}
\begin{document}
\end{document}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);
    let editor = result.editor();
    assert_eq!(editor.frame_count().unwrap(), 0);

    let new_slide = "\\begin{frame}{最初のスライド}\n  内容\n\\end{frame}\n";
    let with_slide = editor
        .insert_frame(0, new_slide)
        .expect("空の本文へのスライド挿入に失敗。");

    assert_eq!(with_slide.frame_count().unwrap(), 1);
    let frames = with_slide.get_frames().unwrap();
    assert_eq!(frames[0].title_text().as_deref(), Some("最初のスライド"));
    assert!(!with_slide.to_parse_result().has_errors());
}

#[test]
fn test_edit_out_of_bounds_and_syntax_error_guards() {
    let source = r#"\documentclass{beamer}
\begin{document}
\begin{frame}{スライド0}\end{frame}
\end{document}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);
    let editor = result.editor();

    // 範囲外のインデックス指定時にエラーとなることを検証する。
    assert!(matches!(
        editor.move_frame(0, 5),
        Err(EditError::IndexOutOfBounds { index: 5, max: 1 })
    ));
    assert!(matches!(
        editor.swap_frames(0, 2),
        Err(EditError::IndexOutOfBounds { index: 2, max: 1 })
    ));
    assert!(matches!(
        editor.delete_frame(1),
        Err(EditError::IndexOutOfBounds { index: 1, max: 1 })
    ));
    assert!(matches!(
        editor.insert_frame(2, "\\begin{frame}\\end{frame}"),
        Err(EditError::IndexOutOfBounds { index: 2, max: 1 })
    ));

    // 構文エラーを含むスライドが存在する場合に編集が拒絶されることを検証する。
    let broken_source = r#"\documentclass{beamer}
\begin{document}
\begin{frame}{壊れたスライド}
  閉じタグなし
\end{document}
"#;
    let broken_editor = parser.parse(broken_source).editor();
    assert!(matches!(
        broken_editor.move_frame(0, 0),
        Err(EditError::SyntaxError { .. })
    ));
    assert!(matches!(
        broken_editor.delete_frame(0),
        Err(EditError::SyntaxError { .. })
    ));
}
