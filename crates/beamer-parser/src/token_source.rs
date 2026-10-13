//! トークン列の走査および先読みを提供する `TokenSource` モジュール。

use crate::lexer::Token;
use crate::syntax::SyntaxKind;

/// 字句解析器から得られたトークン列を保持し、先読みと消費を制御するトークンストリーム。
#[derive(Debug, Clone)]
pub struct TokenSource<'a> {
    tokens: Vec<Token<'a>>,
    cursor: usize,
}

impl<'a> TokenSource<'a> {
    /// 新しい `TokenSource` インスタンスを生成する。
    pub fn new(tokens: Vec<Token<'a>>) -> Self {
        Self { tokens, cursor: 0 }
    }

    /// 現在のカーソルインデックスを取得する。
    #[inline]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// 入力終端である `EOF` に達しているかを判定する。
    #[inline]
    pub fn is_eof(&self) -> bool {
        self.cursor >= self.tokens.len()
    }

    /// 現在のトークンを参照する。
    #[inline]
    pub fn current(&self) -> Option<&Token<'a>> {
        self.tokens.get(self.cursor)
    }

    /// 現在のトークンの `SyntaxKind` を取得する。
    #[inline]
    pub fn current_kind(&self) -> Option<SyntaxKind> {
        self.current().map(|token| token.kind)
    }

    /// 現在位置から相対オフセット `n` だけ先のトークンを参照する。
    #[inline]
    pub fn nth(&self, n: usize) -> Option<&Token<'a>> {
        self.tokens.get(self.cursor + n)
    }

    /// 現在位置から相対オフセット `n` だけ先の `SyntaxKind` を取得する。
    #[inline]
    pub fn nth_kind(&self, n: usize) -> Option<SyntaxKind> {
        self.nth(n).map(|token| token.kind)
    }

    /// 現在のトークンを1つ消費してカーソルを進める。
    #[inline]
    pub fn bump(&mut self) -> Option<Token<'a>> {
        if self.is_eof() {
            None
        } else {
            let token = self.tokens[self.cursor];
            self.cursor += 1;
            Some(token)
        }
    }

    /// 現在位置以降で、指定した相対インデックス `n` 番目の非 `Trivia` トークンを取得する。
    ///
    /// `n = 0` の場合は直近の非 `Trivia` トークンを返す。
    pub fn nth_non_trivia(&self, n: usize) -> Option<&Token<'a>> {
        let mut count = 0;
        let mut idx = self.cursor;
        while idx < self.tokens.len() {
            let token = &self.tokens[idx];
            if !token.kind.is_trivia() {
                if count == n {
                    return Some(token);
                }
                count += 1;
            }
            idx += 1;
        }
        None
    }

    /// 現在位置以降で、指定した相対インデックス `n` 番目の非 `Trivia` トークンの絶対インデックスを取得する。
    pub fn nth_non_trivia_index(&self, n: usize) -> Option<usize> {
        let mut count = 0;
        let mut idx = self.cursor;
        while idx < self.tokens.len() {
            let token = &self.tokens[idx];
            if !token.kind.is_trivia() {
                if count == n {
                    return Some(idx);
                }
                count += 1;
            }
            idx += 1;
        }
        None
    }

    /// 直後の非 `Trivia` トークン列が、指定環境の開始タグ `\begin{env_name}` であるかを判定する。
    pub fn is_at_env_begin(&self, env_name: &str) -> bool {
        let token0 = self.nth_non_trivia(0);
        let token1 = self.nth_non_trivia(1);
        let token2 = self.nth_non_trivia(2);
        let token3 = self.nth_non_trivia(3);

        match (token0, token1, token2, token3) {
            (Some(t0), Some(t1), Some(t2), Some(t3)) => {
                t0.kind == SyntaxKind::Command
                    && t0.text == "\\begin"
                    && t1.kind == SyntaxKind::LBrace
                    && t2.kind == SyntaxKind::Text
                    && t2.text == env_name
                    && t3.kind == SyntaxKind::RBrace
            }
            _ => false,
        }
    }

    /// 直後の非 `Trivia` トークン列が、指定環境の終了タグ `\end{env_name}` であるかを判定する。
    pub fn is_at_env_end(&self, env_name: &str) -> bool {
        let token0 = self.nth_non_trivia(0);
        let token1 = self.nth_non_trivia(1);
        let token2 = self.nth_non_trivia(2);
        let token3 = self.nth_non_trivia(3);

        match (token0, token1, token2, token3) {
            (Some(t0), Some(t1), Some(t2), Some(t3)) => {
                t0.kind == SyntaxKind::Command
                    && t0.text == "\\end"
                    && t1.kind == SyntaxKind::LBrace
                    && t2.kind == SyntaxKind::Text
                    && t2.text == env_name
                    && t3.kind == SyntaxKind::RBrace
            }
            _ => false,
        }
    }

    /// 現在のカーソル位置から、指定環境の開始タグ境界が始まるかを判定する。
    ///
    /// 直前行の改行やコメントが残っている間は `false` を返し、環境開始タグや行頭インデントに到達した時点で `true` を返す。
    pub fn is_at_env_begin_boundary(&self, env_name: &str) -> bool {
        if !self.is_at_env_begin(env_name) {
            return false;
        }
        // 現在のトークンが改行またはコメントである場合は、直前行の要素として扱い境界と判定しない。
        !matches!(
            self.current_kind(),
            Some(SyntaxKind::Newline | SyntaxKind::Comment)
        )
    }

    /// 現在のカーソル位置から、指定環境の終了タグ境界が始まるかを判定する。
    ///
    /// 直前行の改行やコメントが残っている間は `false` を返し、環境終了タグや行頭インデントに到達した時点で `true` を返す。
    pub fn is_at_env_end_boundary(&self, env_name: &str) -> bool {
        if !self.is_at_env_end(env_name) {
            return false;
        }
        // 現在のトークンが改行またはコメントである場合は、直前行の要素として扱い境界と判定しない。
        !matches!(
            self.current_kind(),
            Some(SyntaxKind::Newline | SyntaxKind::Comment)
        )
    }

    /// 現在位置にある環境タグ `\begin{...}` または `\end{...}` の閉じ波括弧 `}` が位置する絶対インデックスを取得する。
    #[inline]
    pub fn env_tag_end_index(&self) -> Option<usize> {
        self.nth_non_trivia_index(3)
    }
}
