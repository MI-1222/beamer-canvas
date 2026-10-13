//! `BeamerParser` の統合テスト。

use beamer_parser::{BeamerParser, SyntaxKind};

#[test]
fn test_parse_roundtrip_basic() {
    let source = "\\documentclass{beamer}\n\\begin{document}\n\\end{document}";
    let parser = BeamerParser::new();
    let result = parser.parse(source);

    assert_eq!(result.syntax().text().to_string(), source);
}

#[test]
fn test_parse_preamble_and_document_body_nodes() {
    let source = r#"\documentclass[10pt,aspectratio=169]{beamer}
\usetheme{Madrid}
\usepackage{amsmath}

\title{テスト発表}
\author{発表者}

\begin{document}
\begin{frame}
  \frametitle{スライド1}
  本文テキスト
\end{frame}
\end{document}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);

    // 入力テキストとの完全一致を検証する。
    assert_eq!(result.syntax().text().to_string(), source);

    // `Preamble` ノードを検証する。
    let preamble = result.preamble().expect("Preamble ノードが存在する。");
    assert_eq!(preamble.kind(), SyntaxKind::Preamble);
    let expected_preamble = r#"\documentclass[10pt,aspectratio=169]{beamer}
\usetheme{Madrid}
\usepackage{amsmath}

\title{テスト発表}
\author{発表者}

"#;
    assert_eq!(preamble.text().to_string(), expected_preamble);

    // `DocumentBody` ノードを検証する。
    let body = result
        .document_body()
        .expect("DocumentBody ノードが存在する。");
    assert_eq!(body.kind(), SyntaxKind::DocumentBody);
    let expected_body = r#"\begin{document}
\begin{frame}
  \frametitle{スライド1}
  本文テキスト
\end{frame}
\end{document}"#;
    assert_eq!(body.text().to_string(), expected_body);
}

#[test]
fn test_parse_preamble_only() {
    let source = r#"% 設定ファイル
\documentclass{beamer}
\usepackage{graphicx}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);

    assert_eq!(result.syntax().text().to_string(), source);

    // `\begin{document}` が存在しない場合、全体が `Preamble` ノードに含まれる。
    let preamble = result.preamble().expect("Preamble ノードが存在する。");
    assert_eq!(preamble.text().to_string(), source);
    assert!(result.document_body().is_none());
}

#[test]
fn test_parse_document_without_end() {
    let source = r#"\documentclass{beamer}
\begin{document}
\begin{frame}
  作成途中のスライド
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);

    assert_eq!(result.syntax().text().to_string(), source);

    let preamble = result.preamble().expect("Preamble ノードが存在する。");
    assert_eq!(preamble.text().to_string(), "\\documentclass{beamer}\n");

    let body = result
        .document_body()
        .expect("DocumentBody ノードが存在する。");
    let expected_body = r#"\begin{document}
\begin{frame}
  作成途中のスライド
"#;
    assert_eq!(body.text().to_string(), expected_body);
}

#[test]
fn test_parse_post_document_content() {
    let source = "\\documentclass{beamer}\n\\begin{document}\n\\end{document}\n% 後続のコメント\n";
    let parser = BeamerParser::new();
    let result = parser.parse(source);

    assert_eq!(result.syntax().text().to_string(), source);

    let body = result
        .document_body()
        .expect("DocumentBody ノードが存在する。");
    assert_eq!(
        body.text().to_string(),
        "\\begin{document}\n\\end{document}"
    );

    // 後続の改行やコメントが `Root` ノード直下に保持されることを確認する。
    assert!(
        result
            .syntax()
            .text()
            .to_string()
            .ends_with("% 後続のコメント\n")
    );
}

#[test]
fn test_parse_empty_and_whitespace() {
    let parser = BeamerParser::new();

    // 空文字列の場合を検証する。
    let empty_result = parser.parse("");
    assert_eq!(empty_result.syntax().text().to_string(), "");
    assert!(empty_result.preamble().is_none());
    assert!(empty_result.document_body().is_none());

    // 空白と改行のみの場合を検証する。
    let ws_source = "   \n\t\n";
    let ws_result = parser.parse(ws_source);
    assert_eq!(ws_result.syntax().text().to_string(), ws_source);
    let preamble = ws_result.preamble().expect("Preamble ノードが存在する。");
    assert_eq!(preamble.text().to_string(), ws_source);
}

#[test]
fn test_parse_complex_preamble_with_macros_and_comments() {
    let source = r#"%!TEX program = lualatex
% 日本語 Beamer プレゼンテーション設定
\documentclass[unicode,12pt]{beamer}

\usetheme{metropolis}
\usecolortheme{owl}

% 独自マクロの定義
\newcommand{\vect}[1]{\boldsymbol{#1}}
\def\realnumbers{\mathbb{R}}

\begin{document}
\end{document}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);

    assert_eq!(result.syntax().text().to_string(), source);

    let preamble = result.preamble().expect("Preamble ノードが存在する。");
    assert!(preamble.text().to_string().contains("\\newcommand{\\vect}"));
    assert!(preamble.text().to_string().contains("% 独自マクロの定義"));
}

#[test]
fn test_parse_indented_document_begin_and_comment_boundary() {
    let source = r#"\documentclass{beamer}
% プリアンブル末尾コメント
  \begin{document}
    \begin{frame}
    \end{frame}
  \end{document}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);

    assert_eq!(result.syntax().text().to_string(), source);

    let preamble = result.preamble().expect("Preamble ノードが存在する。");
    let expected_preamble = "\\documentclass{beamer}\n% プリアンブル末尾コメント\n";
    assert_eq!(preamble.text().to_string(), expected_preamble);

    let body = result
        .document_body()
        .expect("DocumentBody ノードが存在する。");
    assert!(body.text().to_string().starts_with("  \\begin{document}"));
    assert!(body.text().to_string().ends_with("  \\end{document}"));
}

#[test]
fn test_parse_broken_and_unbalanced_preamble() {
    // 波括弧が閉じていないマクロや特殊記号が存在しても、パニックを起こさずに `CST` を構築する。
    let source = r#"\documentclass{beamer
\def\broken{unclosed
$x + y =
\begin{document}
\end{document}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);

    assert_eq!(result.syntax().text().to_string(), source);
    assert!(result.preamble().is_some());
    assert!(result.document_body().is_some());
}

#[test]
fn test_parse_section_and_subsection_nodes() {
    let source = r#"\documentclass{beamer}
\begin{document}
\section{イントロダクション}
\section[略称]{正式セクション名}
\subsection{背景と目的}
\section*{目次非掲載セクション}
\end{document}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);

    // ラウンドトリップ完全一致を検証する。
    assert_eq!(result.syntax().text().to_string(), source);

    let sections = result.sections();
    assert_eq!(sections.len(), 4);

    // 1. `\section{イントロダクション}`
    let s0 = beamer_parser::SectionNode::new(&sections[0]).expect("SectionNode である。");
    assert!(!s0.is_subsection());
    assert_eq!(s0.title_text().as_deref(), Some("イントロダクション"));
    assert_eq!(s0.options_text(), None);

    // 2. `\section[略称]{正式セクション名}`
    let s1 = beamer_parser::SectionNode::new(&sections[1]).expect("SectionNode である。");
    assert!(!s1.is_subsection());
    assert_eq!(s1.title_text().as_deref(), Some("正式セクション名"));
    assert_eq!(s1.options_text().as_deref(), Some("略称"));

    // 3. `\subsection{背景と目的}`
    let s2 = beamer_parser::SectionNode::new(&sections[2]).expect("SectionNode である。");
    assert!(s2.is_subsection());
    assert_eq!(s2.title_text().as_deref(), Some("背景と目的"));

    // 4. `\section*{目次非掲載セクション}`
    let s3 = beamer_parser::SectionNode::new(&sections[3]).expect("SectionNode である。");
    assert!(!s3.is_subsection());
    assert_eq!(s3.title_text().as_deref(), Some("目次非掲載セクション"));
}

#[test]
fn test_parse_frame_environment_with_header_variations() {
    let source = r#"\documentclass{beamer}
\begin{document}
\begin{frame}<1->[plain,t]{スライドタイトル}{サブタイトル}
  スライド本文 1
\end{frame}

\begin{frame}[fragile]{コード解説}
  \begin{verbatim}
  println!("Hello");
  \end{verbatim}
\end{frame}

\begin{frame}{タイトルのみ}
  本文 3
\end{frame}

\begin{frame}
  \frametitle{本文内タイトルコマンド}
  本文 4
\end{frame}
\end{document}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);

    assert_eq!(result.syntax().text().to_string(), source);

    let frames = result.frames();
    assert_eq!(frames.len(), 4);

    // 1. `<1->[plain,t]{スライドタイトル}{サブタイトル}`
    let f0 = beamer_parser::FrameNode::new(&frames[0]).expect("FrameNode である。");
    assert_eq!(f0.overlay_spec_text().as_deref(), Some("1-"));
    assert_eq!(f0.options_text().as_deref(), Some("plain,t"));
    assert_eq!(f0.title_text().as_deref(), Some("スライドタイトル"));
    assert_eq!(f0.subtitle_text().as_deref(), Some("サブタイトル"));

    // 2. `[fragile]{コード解説}`
    let f1 = beamer_parser::FrameNode::new(&frames[1]).expect("FrameNode である。");
    assert_eq!(f1.overlay_spec_text(), None);
    assert_eq!(f1.options_text().as_deref(), Some("fragile"));
    assert_eq!(f1.title_text().as_deref(), Some("コード解説"));
    assert_eq!(f1.subtitle_text(), None);

    // 3. `{タイトルのみ}`
    let f2 = beamer_parser::FrameNode::new(&frames[2]).expect("FrameNode である。");
    assert_eq!(f2.options_text(), None);
    assert_eq!(f2.title_text().as_deref(), Some("タイトルのみ"));

    // 4. `\frametitle{本文内タイトルコマンド}` によるフォールバック取得。
    let f3 = beamer_parser::FrameNode::new(&frames[3]).expect("FrameNode である。");
    assert_eq!(f3.title_text().as_deref(), Some("本文内タイトルコマンド"));
}

#[test]
fn test_parse_legacy_frame_macro() {
    let source = r#"\documentclass{beamer}
\begin{document}
\frame{
  旧式マクロのフレーム本文
}

\frame[plain]{
  オプション付き旧式フレーム
}
\end{document}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);

    assert_eq!(result.syntax().text().to_string(), source);

    let frames = result.frames();
    assert_eq!(frames.len(), 2);

    let f0 = beamer_parser::FrameNode::new(&frames[0]).expect("FrameNode である。");
    assert!(
        f0.syntax()
            .text()
            .to_string()
            .contains("旧式マクロのフレーム本文")
    );

    let f1 = beamer_parser::FrameNode::new(&frames[1]).expect("FrameNode である。");
    assert_eq!(f1.options_text().as_deref(), Some("plain"));
}

#[test]
fn test_parse_nested_options_and_title_with_macros() {
    let source = r#"\documentclass{beamer}
\begin{document}
\begin{frame}[fragile, environment={customframe}]{\textbf{太字}タイトル $x^2$}
  本文
\end{frame}
\end{document}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);

    assert_eq!(result.syntax().text().to_string(), source);

    let frames = result.frames();
    assert_eq!(frames.len(), 1);

    let f0 = beamer_parser::FrameNode::new(&frames[0]).expect("FrameNode である。");
    assert_eq!(
        f0.options_text().as_deref(),
        Some("fragile, environment={customframe}")
    );
    assert_eq!(
        f0.title_text().as_deref(),
        Some(r#"\textbf{太字}タイトル $x^2$"#)
    );
}

#[test]
fn test_parse_frame_trailing_trivia_retention() {
    let source = "\\documentclass{beamer}\n\\begin{document}\n\\begin{frame}\n  本文\n\\end{frame}\n\\end{document}\n";

    let parser = BeamerParser::new();
    let result = parser.parse(source);

    assert_eq!(result.syntax().text().to_string(), source);

    let frames = result.frames();
    assert_eq!(frames.len(), 1);

    // `Frame` ノードのテキストが直後の単一改行を含むことを検証する。
    let frame_text = frames[0].text().to_string();
    assert!(frame_text.ends_with("\n"));
    assert_eq!(frame_text, "\\begin{frame}\n  本文\n\\end{frame}\n");
}

#[test]
fn test_parse_unclosed_frame_error_recovery() {
    let source = r#"\documentclass{beamer}
\begin{document}
\begin{frame}
  閉じタグのないスライド
\begin{frame}
  正常なスライド
\end{frame}
\end{document}
"#;

    let parser = BeamerParser::new();
    let result = parser.parse(source);

    // パニックせず、入力全体をロスレスに保持する。
    assert_eq!(result.syntax().text().to_string(), source);

    let frames = result.frames();
    assert_eq!(frames.len(), 2);
    assert!(!result.errors.is_empty());
}
