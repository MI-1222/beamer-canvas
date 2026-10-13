//! 手書き決定性ステートマシンによるゼロコピー字句解析器 `Lexer` モジュール。

use crate::cursor::Cursor;
use crate::syntax::SyntaxKind;

/// 字句解析で切り出した単一の `Token`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token<'a> {
    /// `Token` の構文種別。
    pub kind: SyntaxKind,
    /// 入力文字列から切り出した `Token` の生スライス。
    pub text: &'a str,
}

impl<'a> Token<'a> {
    /// 新しい `Token` インスタンスを生成する。
    #[inline]
    pub const fn new(kind: SyntaxKind, text: &'a str) -> Self {
        Self { kind, text }
    }

    /// `Token` のバイト長を取得する。
    #[inline]
    pub const fn len(&self) -> usize {
        self.text.len()
    }

    /// `Token` が空であるかを判定する。
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// 入力文字列から `Token` 列を逐次生成する `Lexer` 構造体。
#[derive(Debug, Clone)]
pub struct Lexer<'a> {
    cursor: Cursor<'a>,
}

impl<'a> Lexer<'a> {
    /// 新しい `Lexer` インスタンスを生成する。
    #[inline]
    pub fn new(input: &'a str) -> Self {
        Self {
            cursor: Cursor::new(input),
        }
    }

    /// 次の `Token` を1つ切り出す。
    ///
    /// 入力終端に達した場合は `None` を返す。
    pub fn next_token(&mut self) -> Option<Token<'a>> {
        if self.cursor.is_eof() {
            return None;
        }

        let start_bytes = self.cursor.consumed_bytes();
        let rest = self.cursor.rest();
        let kind = self.scan_token_kind();
        let end_bytes = self.cursor.consumed_bytes();

        let len = end_bytes - start_bytes;

        // 1文字も消費されなかった場合の無限ループ防止用フォールバック。
        if len == 0 {
            if let Some(ch) = self.cursor.bump() {
                let token_text = &rest[..ch.len_utf8()];
                return Some(Token::new(SyntaxKind::Error, token_text));
            }
            return None;
        }

        let text = &rest[..len];
        Some(Token::new(kind, text))
    }

    /// 先頭文字に応じた `SyntaxKind` を判定し、カーソルを進める。
    fn scan_token_kind(&mut self) -> SyntaxKind {
        let first = self.cursor.first();
        match first {
            // 水平空白(`Whitespace`)。
            ' ' | '\t' => {
                self.cursor.consume_while(|c| c == ' ' || c == '\t');
                SyntaxKind::Whitespace
            }
            // 改行(`LF` または `CRLF`)。
            '\n' => {
                self.cursor.bump();
                SyntaxKind::Newline
            }
            '\r' => {
                self.cursor.bump();
                if self.cursor.first() == '\n' {
                    self.cursor.bump();
                }
                SyntaxKind::Newline
            }
            // コメント(`%` から行末手前まで)。
            '%' => {
                self.cursor.bump();
                self.cursor.consume_while(|c| c != '\n' && c != '\r');
                SyntaxKind::Comment
            }
            // バックスラッシュ(`Command`、`EscapedChar`、`NewlineCommand`)。
            '\\' => {
                self.cursor.bump();
                let second = self.cursor.first();
                if second == '\0' {
                    // 入力末尾の単独バックスラッシュ。
                    SyntaxKind::Error
                } else if second == '\\' {
                    // 改行コマンド `\\`。
                    self.cursor.bump();
                    SyntaxKind::NewlineCommand
                } else if second.is_ascii_alphabetic() {
                    // 制御語(`\begin`, `\item` 等)。
                    self.cursor.consume_while(|c| c.is_ascii_alphabetic());
                    SyntaxKind::Command
                } else {
                    // 制御符・エスケープ文字(`\%`, `\{`, `\ `, `\_` 等)。
                    self.cursor.bump();
                    SyntaxKind::EscapedChar
                }
            }
            // 括弧・デリミタ。
            '{' => {
                self.cursor.bump();
                SyntaxKind::LBrace
            }
            '}' => {
                self.cursor.bump();
                SyntaxKind::RBrace
            }
            '[' => {
                self.cursor.bump();
                SyntaxKind::LBracket
            }
            ']' => {
                self.cursor.bump();
                SyntaxKind::RBracket
            }
            '<' => {
                self.cursor.bump();
                SyntaxKind::LAngle
            }
            '>' => {
                self.cursor.bump();
                SyntaxKind::RAngle
            }
            // 数式記号(`$` または `$$`)。
            '$' => {
                self.cursor.bump();
                if self.cursor.first() == '$' {
                    self.cursor.bump();
                    SyntaxKind::DoubleDollar
                } else {
                    SyntaxKind::Dollar
                }
            }
            // 特殊文字。
            '&' => {
                self.cursor.bump();
                SyntaxKind::Ampersand
            }
            '_' => {
                self.cursor.bump();
                SyntaxKind::Underscore
            }
            '^' => {
                self.cursor.bump();
                SyntaxKind::Caret
            }
            '~' => {
                self.cursor.bump();
                SyntaxKind::Tilde
            }
            '#' => {
                self.cursor.bump();
                SyntaxKind::Hash
            }
            // 一般テキスト。
            _ => {
                self.cursor.consume_while(|c| !is_delimiter_or_special(c));
                SyntaxKind::Text
            }
        }
    }
}

/// 指定した文字がデリミタまたは特殊文字であるかを判定する。
#[inline]
#[allow(clippy::match_like_matches_macro)]
const fn is_delimiter_or_special(c: char) -> bool {
    match c {
        ' ' | '\t' | '\n' | '\r' | '%' | '\\' | '{' | '}' | '[' | ']' | '<' | '>' | '$' | '&'
        | '_' | '^' | '~' | '#' => true,
        _ => false,
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Token<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.next_token()
    }
}

/// 入力文字列を `Token` 列へ分解するヘルパー関数。
pub fn tokenize(input: &str) -> Vec<Token<'_>> {
    Lexer::new(input).collect()
}
