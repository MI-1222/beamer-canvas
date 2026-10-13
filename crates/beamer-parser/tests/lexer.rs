//! 手書き字句解析器(`Lexer`)の単体テストおよびラウンドトリップ検証。

use beamer_parser::{SyntaxKind, tokenize};

/// トークン列を連結して元の入力文字列と完全一致するかを検証する。
fn assert_roundtrip(source: &str) {
    let tokens = tokenize(source);
    let restored: String = tokens.iter().map(|t| t.text).collect();
    assert_eq!(
        source, restored,
        "ラウンドトリップ完全一致が保証されていません。"
    );
}

#[test]
fn test_whitespace_and_newlines() {
    let source = "  \t \n\r\n \r";
    assert_roundtrip(source);

    let tokens = tokenize(source);
    assert_eq!(tokens[0].kind, SyntaxKind::Whitespace);
    assert_eq!(tokens[0].text, "  \t ");
    assert_eq!(tokens[1].kind, SyntaxKind::Newline);
    assert_eq!(tokens[1].text, "\n");
    assert_eq!(tokens[2].kind, SyntaxKind::Newline);
    assert_eq!(tokens[2].text, "\r\n");
    assert_eq!(tokens[3].kind, SyntaxKind::Whitespace);
    assert_eq!(tokens[3].text, " ");
    assert_eq!(tokens[4].kind, SyntaxKind::Newline);
    assert_eq!(tokens[4].text, "\r");
}

#[test]
fn test_comment_separation_from_newline() {
    let source = "% これはコメントです\n% 2行目\r\n最後の行";
    assert_roundtrip(source);

    let tokens = tokenize(source);
    assert_eq!(tokens[0].kind, SyntaxKind::Comment);
    assert_eq!(tokens[0].text, "% これはコメントです");
    assert_eq!(tokens[1].kind, SyntaxKind::Newline);
    assert_eq!(tokens[1].text, "\n");
    assert_eq!(tokens[2].kind, SyntaxKind::Comment);
    assert_eq!(tokens[2].text, "% 2行目");
    assert_eq!(tokens[3].kind, SyntaxKind::Newline);
    assert_eq!(tokens[3].text, "\r\n");
    assert_eq!(tokens[4].kind, SyntaxKind::Text);
    assert_eq!(tokens[4].text, "最後の行");
}

#[test]
fn test_empty_and_trailing_comments() {
    let source = "%\n%\n%";
    assert_roundtrip(source);

    let tokens = tokenize(source);
    assert_eq!(tokens[0].kind, SyntaxKind::Comment);
    assert_eq!(tokens[0].text, "%");
    assert_eq!(tokens[1].kind, SyntaxKind::Newline);
    assert_eq!(tokens[2].kind, SyntaxKind::Comment);
    assert_eq!(tokens[2].text, "%");
    assert_eq!(tokens[3].kind, SyntaxKind::Newline);
    assert_eq!(tokens[4].kind, SyntaxKind::Comment);
    assert_eq!(tokens[4].text, "%");
}

#[test]
fn test_commands_and_escapes() {
    let source = r"\documentclass{beamer} \% \{ \} \_ \& \# \$ \^ \~ \ ";
    assert_roundtrip(source);

    let tokens = tokenize(source);
    assert_eq!(tokens[0].kind, SyntaxKind::Command);
    assert_eq!(tokens[0].text, r"\documentclass");
    assert_eq!(tokens[1].kind, SyntaxKind::LBrace);
    assert_eq!(tokens[2].kind, SyntaxKind::Text);
    assert_eq!(tokens[2].text, "beamer");
    assert_eq!(tokens[3].kind, SyntaxKind::RBrace);

    // 各エスケープ文字が `EscapedChar` として切り出されることを確認。
    let escaped_tokens: Vec<&str> = tokens
        .iter()
        .filter(|t| t.kind == SyntaxKind::EscapedChar)
        .map(|t| t.text)
        .collect();
    assert_eq!(
        escaped_tokens,
        vec![
            r"\%", r"\{", r"\}", r"\_", r"\&", r"\#", r"\$", r"\^", r"\~", r"\ "
        ]
    );
}

#[test]
fn test_newline_command_boundary() {
    let source = r"\\[5pt]";
    assert_roundtrip(source);

    let tokens = tokenize(source);
    assert_eq!(tokens[0].kind, SyntaxKind::NewlineCommand);
    assert_eq!(tokens[0].text, r"\\");
    assert_eq!(tokens[1].kind, SyntaxKind::LBracket);
    assert_eq!(tokens[1].text, "[");
    assert_eq!(tokens[2].kind, SyntaxKind::Text);
    assert_eq!(tokens[2].text, "5pt");
    assert_eq!(tokens[3].kind, SyntaxKind::RBracket);
    assert_eq!(tokens[3].text, "]");
}

#[test]
fn test_math_dollar_and_double_dollar() {
    let source = r"$x + y$ and $$E = mc^2$$";
    assert_roundtrip(source);

    let tokens = tokenize(source);
    assert_eq!(tokens[0].kind, SyntaxKind::Dollar);
    assert_eq!(tokens[0].text, "$");
    assert_eq!(tokens[1].kind, SyntaxKind::Text);
    assert_eq!(tokens[1].text, "x");
    assert_eq!(tokens[2].kind, SyntaxKind::Whitespace);
    assert_eq!(tokens[2].text, " ");
    assert_eq!(tokens[3].kind, SyntaxKind::Text);
    assert_eq!(tokens[3].text, "+");
    assert_eq!(tokens[4].kind, SyntaxKind::Whitespace);
    assert_eq!(tokens[4].text, " ");
    assert_eq!(tokens[5].kind, SyntaxKind::Text);
    assert_eq!(tokens[5].text, "y");
    assert_eq!(tokens[6].kind, SyntaxKind::Dollar);
    assert_eq!(tokens[6].text, "$");

    assert_eq!(tokens[7].kind, SyntaxKind::Whitespace);
    assert_eq!(tokens[8].kind, SyntaxKind::Text);
    assert_eq!(tokens[8].text, "and");
    assert_eq!(tokens[9].kind, SyntaxKind::Whitespace);

    assert_eq!(tokens[10].kind, SyntaxKind::DoubleDollar);
    assert_eq!(tokens[10].text, "$$");
    assert_eq!(tokens[11].kind, SyntaxKind::Text);
    assert_eq!(tokens[11].text, "E");
    assert_eq!(tokens[12].kind, SyntaxKind::Whitespace);
    assert_eq!(tokens[13].kind, SyntaxKind::Text);
    assert_eq!(tokens[13].text, "=");
    assert_eq!(tokens[14].kind, SyntaxKind::Whitespace);
    assert_eq!(tokens[15].kind, SyntaxKind::Text);
    assert_eq!(tokens[15].text, "mc");
    assert_eq!(tokens[16].kind, SyntaxKind::Caret);
    assert_eq!(tokens[16].text, "^");
    assert_eq!(tokens[17].kind, SyntaxKind::Text);
    assert_eq!(tokens[17].text, "2");
    assert_eq!(tokens[18].kind, SyntaxKind::DoubleDollar);
    assert_eq!(tokens[18].text, "$$");
}

#[test]
fn test_overlay_spec_symbols() {
    let source = r"\only<1-2>{内容}";
    assert_roundtrip(source);

    let tokens = tokenize(source);
    assert_eq!(tokens[0].kind, SyntaxKind::Command);
    assert_eq!(tokens[0].text, r"\only");
    assert_eq!(tokens[1].kind, SyntaxKind::LAngle);
    assert_eq!(tokens[1].text, "<");
    assert_eq!(tokens[2].kind, SyntaxKind::Text);
    assert_eq!(tokens[2].text, "1-2");
    assert_eq!(tokens[3].kind, SyntaxKind::RAngle);
    assert_eq!(tokens[3].text, ">");
    assert_eq!(tokens[4].kind, SyntaxKind::LBrace);
    assert_eq!(tokens[5].kind, SyntaxKind::Text);
    assert_eq!(tokens[5].text, "内容");
    assert_eq!(tokens[6].kind, SyntaxKind::RBrace);
}

#[test]
fn test_multibyte_japanese_and_fullwidth() {
    let source = "\\frametitle{日本語プレゼンテーション：概要と方針}\nスライド本文「引用」";
    assert_roundtrip(source);

    let tokens = tokenize(source);
    assert_eq!(tokens[0].kind, SyntaxKind::Command);
    assert_eq!(tokens[0].text, r"\frametitle");
    assert_eq!(tokens[1].kind, SyntaxKind::LBrace);
    assert_eq!(tokens[2].kind, SyntaxKind::Text);
    assert_eq!(tokens[2].text, "日本語プレゼンテーション：概要と方針");
    assert_eq!(tokens[3].kind, SyntaxKind::RBrace);
    assert_eq!(tokens[4].kind, SyntaxKind::Newline);
    assert_eq!(tokens[5].kind, SyntaxKind::Text);
    assert_eq!(tokens[5].text, "スライド本文「引用」");
}

#[test]
fn test_incomplete_backslash_at_eof() {
    let source = r"\";
    assert_roundtrip(source);

    let tokens = tokenize(source);
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, SyntaxKind::Error);
    assert_eq!(tokens[0].text, r"\");
}

#[test]
fn test_complex_beamer_snippet() {
    let source = r#"\begin{frame}[t,fragile]<1->{タイトル}{サブタイトル}
  % 箇条書きの例
  \begin{itemize}
    \item 項目 1: $f(x) = \sin(x)$
    \item<2-> 項目 2: 特別な記号 \% \& \_
  \end{itemize}
\end{frame}
"#;
    assert_roundtrip(source);
}
