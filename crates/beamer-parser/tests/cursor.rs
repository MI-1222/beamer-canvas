//! カーソル(`Cursor`)モジュールの単体テスト。

use beamer_parser::Cursor;

#[test]
fn test_cursor_basic_navigation() {
    let text = "abc";
    let mut cursor = Cursor::new(text);

    assert_eq!(cursor.first(), 'a');
    assert_eq!(cursor.second(), 'b');
    assert_eq!(cursor.consumed_bytes(), 0);
    assert_eq!(cursor.rest(), "abc");

    assert_eq!(cursor.bump(), Some('a'));
    assert_eq!(cursor.first(), 'b');
    assert_eq!(cursor.second(), 'c');
    assert_eq!(cursor.consumed_bytes(), 1);
    assert_eq!(cursor.rest(), "bc");

    assert_eq!(cursor.bump(), Some('b'));
    assert_eq!(cursor.bump(), Some('c'));
    assert_eq!(cursor.bump(), None);
    assert!(cursor.is_eof());
    assert_eq!(cursor.first(), '\0');
    assert_eq!(cursor.second(), '\0');
    assert_eq!(cursor.consumed_bytes(), 3);
    assert_eq!(cursor.rest(), "");
}

#[test]
fn test_cursor_multibyte_utf8() {
    let text = "こんにちは";
    let mut cursor = Cursor::new(text);

    assert_eq!(cursor.first(), 'こ');
    assert_eq!(cursor.second(), 'ん');
    assert_eq!(cursor.bump(), Some('こ'));
    assert_eq!(cursor.consumed_bytes(), 'こ'.len_utf8());
    assert_eq!(cursor.rest(), "んにちは");
}

#[test]
fn test_cursor_consume_while() {
    let text = "   \tfoo";
    let mut cursor = Cursor::new(text);

    cursor.consume_while(|c| c == ' ' || c == '\t');
    assert_eq!(cursor.consumed_bytes(), 4);
    assert_eq!(cursor.rest(), "foo");
    assert_eq!(cursor.first(), 'f');
}
