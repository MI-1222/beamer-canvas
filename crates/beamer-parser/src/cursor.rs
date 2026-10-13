//! 文字列スライスを安全かつゼロコピーで走査する `Cursor` モジュール。

use std::str::Chars;

/// 入力文字列ストリームを1文字ずつ覗き見および消費しながら走査する `Cursor` 構造体。
#[derive(Debug, Clone)]
pub struct Cursor<'a> {
    /// 初期の全体文字列。
    initial: &'a str,
    /// 未消費の残り文字列を走査する文字イテレータ。
    chars: Chars<'a>,
}

impl<'a> Cursor<'a> {
    /// 新しい `Cursor` インスタンスを生成する。
    #[inline]
    pub fn new(input: &'a str) -> Self {
        Self {
            initial: input,
            chars: input.chars(),
        }
    }

    /// 現在の先頭文字を覗き見る。
    ///
    /// 終端に達している場合は `\0` を返す。
    #[inline]
    pub fn first(&self) -> char {
        self.chars.clone().next().unwrap_or('\0')
    }

    /// 先頭から2番目の文字を覗き見る。
    ///
    /// 終端に達している場合は `\0` を返す。
    #[inline]
    pub fn second(&self) -> char {
        let mut iter = self.chars.clone();
        iter.next();
        iter.next().unwrap_or('\0')
    }

    /// 入力文字列の終端に達しているかを判定する。
    #[inline]
    pub fn is_eof(&self) -> bool {
        self.chars.as_str().is_empty()
    }

    /// 未消費の残り文字列スライスを取得する。
    #[inline]
    pub fn rest(&self) -> &'a str {
        self.chars.as_str()
    }

    /// 入力先頭から現在地までに消費したバイト数を取得する。
    #[inline]
    pub fn consumed_bytes(&self) -> usize {
        self.initial.len() - self.chars.as_str().len()
    }

    /// 1文字消費し、その文字を返す。
    ///
    /// 終端に達している場合は `None` を返す。
    #[inline]
    pub fn bump(&mut self) -> Option<char> {
        self.chars.next()
    }

    /// 指定した述語を満たす間、文字を連続して消費する。
    pub fn consume_while<F>(&mut self, mut predicate: F)
    where
        F: FnMut(char) -> bool,
    {
        while !self.is_eof() && predicate(self.first()) {
            self.bump();
        }
    }
}
