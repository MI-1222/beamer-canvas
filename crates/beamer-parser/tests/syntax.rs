//! 構文要素(`SyntaxKind`)および言語定義の単体テスト。

use beamer_parser::SyntaxKind;

#[test]
fn test_syntax_kind_roundtrip() {
    for raw in 0..(SyntaxKind::__LAST as u16) {
        let rowan_kind = rowan::SyntaxKind(raw);
        let kind = SyntaxKind::from_raw(rowan_kind);
        let converted: rowan::SyntaxKind = kind.into();
        assert_eq!(converted, rowan_kind);
    }
}

#[test]
fn test_syntax_kind_predicates() {
    assert!(SyntaxKind::Whitespace.is_trivia());
    assert!(SyntaxKind::Newline.is_trivia());
    assert!(SyntaxKind::Comment.is_trivia());
    assert!(!SyntaxKind::Text.is_trivia());

    assert!(SyntaxKind::Whitespace.is_whitespace());
    assert!(SyntaxKind::Newline.is_whitespace());
    assert!(!SyntaxKind::Comment.is_whitespace());

    assert!(SyntaxKind::Whitespace.is_token());
    assert!(SyntaxKind::Command.is_token());
    assert!(SyntaxKind::Error.is_token());
    assert!(!SyntaxKind::Root.is_token());
    assert!(!SyntaxKind::Frame.is_token());

    assert!(SyntaxKind::Root.is_node());
    assert!(SyntaxKind::Frame.is_node());
    assert!(SyntaxKind::OpaqueBlock.is_node());
    assert!(!SyntaxKind::Whitespace.is_node());
    assert!(!SyntaxKind::Error.is_node());

    assert!(SyntaxKind::OpaqueBlock.is_opaque_or_error());
    assert!(SyntaxKind::OpaqueMacro.is_opaque_or_error());
    assert!(SyntaxKind::TikzPicture.is_opaque_or_error());
    assert!(SyntaxKind::ErrorNode.is_opaque_or_error());
    assert!(!SyntaxKind::Frame.is_opaque_or_error());
}
