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
