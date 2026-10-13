//! `rowan` に基づく構文要素（SyntaxKind）および言語定義モジュール。

/// LaTeX Beamer ドキュメントを表現する構文要素種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
    /// 空白文字（空白、タブ）。
    Whitespace = 0,
    /// 改行文字。
    Newline,
    /// コメント行（%から行末まで）。
    Comment,
    /// テキストトークン。
    Text,
    /// コマンド（\documentclass, \usetheme, \section 等）。
    Command,
    /// 環境開始（\begin{...}）。
    BeginEnv,
    /// 環境終了（\end{...}）。
    EndEnv,
    /// 中括弧 `{`。
    LBrace,
    /// 中括弧 `}`。
    RBrace,
    /// 角括弧 `[`。
    LBracket,
    /// 角括弧 `]`。
    RBracket,
    /// オーバーレイスペック（`<1->` 等）。
    OverlaySpec,
    /// 不透明な LaTeX ブロック（未解析・未知マクロ領域）。
    OpaqueBlock,

    // 複合ノード（Composite Nodes）
    /// ドキュメントルートノード。
    Root,
    /// プリアンブルノード。
    Preamble,
    /// セクション境界ノード。
    Section,
    /// スライドフレームノード（\begin{frame}...\end{frame}）。
    Frame,
    /// 一般的な環境ノード（itemize, block 等）。
    Environment,
    /// エラーノード。
    Error,
}

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        Self(kind as u16)
    }
}

/// `rowan` における LaTeX 言語の定義型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LatexLanguage {}

impl rowan::Language for LatexLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        assert!(raw.0 <= SyntaxKind::Error as u16);
        unsafe { std::mem::transmute(raw.0) }
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

/// LaTeX 構文ツリーのノード型。
pub type SyntaxNode = rowan::SyntaxNode<LatexLanguage>;
/// LaTeX 構文ツリーのトークン型。
pub type SyntaxToken = rowan::SyntaxToken<LatexLanguage>;
/// LaTeX 構文ツリーの要素型（ノードまたはトークン）。
pub type SyntaxElement = rowan::SyntaxElement<LatexLanguage>;
/// LaTeX 構文ツリーのノード参照型。
pub type SyntaxNodePtr = rowan::ast::SyntaxNodePtr<LatexLanguage>;
