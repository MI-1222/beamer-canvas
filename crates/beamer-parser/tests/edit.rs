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

    let frames = editor.get_frames().expect("スライド抽出が成功する。");
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
    let sections = editor.get_sections().expect("セクション抽出が成功する。");
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
    let frames = result.get_frames().expect("スライド抽出が成功する。");

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

    let frame0 = editor.get_frame(0).expect("インデックス 0 が取得できる。");
    assert_eq!(frame0.title_text().as_deref(), Some("スライド1"));

    let frame1 = editor.get_frame(1).expect("インデックス 1 が取得できる。");
    assert_eq!(frame1.title_text().as_deref(), Some("スライド2"));

    let err_frame = editor.get_frame(2).unwrap_err();
    assert_eq!(err_frame, EditError::IndexOutOfBounds { index: 2, max: 2 });

    let sec0 = editor.get_section(0).expect("セクション 0 が取得できる。");
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
    let frames = result.get_frames().expect("スライド抽出が成功する。");

    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].index(), 0);
    assert_eq!(frames[1].index(), 1);
    assert_eq!(frames[1].title_text().as_deref(), Some("通常スライド"));
}
