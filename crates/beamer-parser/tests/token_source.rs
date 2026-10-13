//! `TokenSource` の統合テスト。

use beamer_parser::{SyntaxKind, TokenSource, tokenize};

#[test]
fn test_token_source_basic() {
    let input = "\\begin{document}\nhello";
    let tokens = tokenize(input);
    let mut source = TokenSource::new(tokens);

    assert!(source.is_at_env_begin("document"));
    assert!(!source.is_at_env_end("document"));

    let end_idx = source.env_tag_end_index().expect("閉じ括弧が存在する。");
    while source.cursor() <= end_idx {
        source.bump();
    }

    assert_eq!(source.current_kind(), Some(SyntaxKind::Newline));
    source.bump();
    assert_eq!(source.current_kind(), Some(SyntaxKind::Text));
    assert_eq!(source.current().map(|t| t.text), Some("hello"));
}

#[test]
fn test_token_source_with_trivia_in_env_tag() {
    let input = "\\begin \n { document }";
    let tokens = tokenize(input);
    let source = TokenSource::new(tokens);

    assert!(source.is_at_env_begin("document"));
}

#[test]
fn test_token_source_boundary_detection() {
    let input = "\\documentclass{beamer}\n% コメント\n  \\begin{document}";
    let tokens = tokenize(input);
    let mut source = TokenSource::new(tokens);

    // 先頭位置では環境開始タグと判定しない。
    assert!(!source.is_at_env_begin("document"));
    assert!(!source.is_at_env_begin_boundary("document"));

    // `\documentclass{beamer}` まで消費する。
    while source.current().map(|t| t.text) != Some("\n") {
        source.bump();
    }

    // 改行位置では `is_at_env_begin` が `true` でも、改行トークン自体の走査中は境界と判定しない。
    assert!(source.is_at_env_begin("document"));
    assert!(!source.is_at_env_begin_boundary("document"));
    source.bump(); // `\n` を消費

    // コメントトークンの走査中も境界と判定しない。
    assert!(source.is_at_env_begin("document"));
    assert!(!source.is_at_env_begin_boundary("document"));
    source.bump(); // `% コメント` を消費
    source.bump(); // `\n` を消費

    // インデント空白に到達した時点で環境開始タグの境界と判定する。
    assert!(source.is_at_env_begin("document"));
    assert!(source.is_at_env_begin_boundary("document"));
}
