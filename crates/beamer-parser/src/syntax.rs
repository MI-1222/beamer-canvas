//! `rowan` に基づく構文要素(`SyntaxKind`)および言語定義モジュール。

/// LaTeX Beamer ドキュメントを構成する構文要素種別。
///
/// 終端記号(トークン)と非終端記号(複合ノード)の双方を同一の列挙型で表現する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
#[allow(clippy::manual_non_exhaustive)]
pub enum SyntaxKind {
    // ------------------------------------------------------------------------
    // Trivia(付随情報トークン)
    // ------------------------------------------------------------------------
    /// 水平空白(半角スペース、タブ)。
    Whitespace = 0,
    /// 改行文字(`\n`, `\r\n`)。
    Newline,
    /// コメント行(`%` から行末手前まで)。
    Comment,

    // ------------------------------------------------------------------------
    // 記号・デリミタ・特殊文字トークン
    // ------------------------------------------------------------------------
    /// 開き波括弧 `{`。
    LBrace,
    /// 閉じ波括弧 `}`。
    RBrace,
    /// 開き角括弧 `[`。
    LBracket,
    /// 閉じ角括弧 `]`。
    RBracket,
    /// 開き山括弧 `<`。
    LAngle,
    /// 閉じ山括弧 `>`。
    RAngle,
    /// 単一ドル記号 `$`。
    Dollar,
    /// 二重ドル記号 `$$`。
    DoubleDollar,
    /// アンパサンド `&`。
    Ampersand,
    /// アンダースコア `_`。
    Underscore,
    /// サーカムフレックス `^`。
    Caret,
    /// チルダ `~`。
    Tilde,
    /// ハッシュ記号 `#`。
    Hash,

    // ------------------------------------------------------------------------
    // コマンド・制御記号・テキストトークン
    // ------------------------------------------------------------------------
    /// 制御綴・マクロ(`\documentclass`, `\begin`, `\item` 等)。
    Command,
    /// 制御符・エスケープ文字(`\%`, `\{`, `\_` 等)。
    EscapedChar,
    /// 改行コマンド(`\\`)。
    NewlineCommand,
    /// 通常のプレーンテキスト。
    Text,
    /// 未知または構文不正な文字トークン。
    Error,

    // ------------------------------------------------------------------------
    // ドキュメント骨格ノード(複合ノード)
    // ------------------------------------------------------------------------
    /// ドキュメントルートノード。
    Root,
    /// プリアンブル領域ノード(`\documentclass` から `\begin{document}` 直前まで)。
    Preamble,
    /// ドキュメント本文ノード(`\begin{document}` から `\end{document}` まで)。
    DocumentBody,

    // ------------------------------------------------------------------------
    // アイランド構文ノード(複合ノード)
    // ------------------------------------------------------------------------
    /// スライドフレームノード(`\begin{frame}...\end{frame}` または `\frame{...}`)。
    Frame,
    /// フレームヘッダーノード(オプション、タイトル等の引数群)。
    FrameHeader,
    /// フレームの角括弧オプションノード(`[plain,t]` 等)。
    FrameOptions,
    /// フレームタイトル引数ノード。
    FrameTitle,
    /// フレームサブタイトル引数ノード。
    FrameSubtitle,
    /// オーバーレイスペックノード(`<1->`, `<alert@2>` 等)。
    OverlaySpec,
    /// セクション境界ノード(`\section{...}`)。
    Section,
    /// サブセクション境界ノード(`\subsection{...}`)。
    Subsection,
    /// ブロック環境ノード(`block`, `exampleblock`, `alertblock` 等)。
    Block,
    /// 複数列環境ノード(`columns`)。
    Columns,
    /// 単一列ノード(`column`)。
    Column,
    /// 箇条書き環境ノード(`itemize`)。
    Itemize,
    /// 番号付き箇条書き環境ノード(`enumerate`)。
    Enumerate,
    /// 箇条書き要素ノード(`\item`)。
    Item,
    /// 角括弧オプションリストノード(`[...]`)。
    OptionsList,
    /// 中括弧引数ノード(`{...}`)。
    Argument,
    /// 一般的な環境ノード(`\begin{env}...\end{env}`)。
    Environment,

    // ------------------------------------------------------------------------
    // 不透明ブロック・エラーノード(複合ノード)
    // ------------------------------------------------------------------------
    /// 一般的な不透明ブロックノード(未解析領域)。
    OpaqueBlock,
    /// 不透明マクロノード(未解析のマクロ呼び出し)。
    OpaqueMacro,
    /// 不透明環境ノード(未解析の環境)。
    OpaqueEnv,
    /// TikZ 描画環境ノード(`tikzpicture`)。
    TikzPicture,
    /// パースエラーノード。
    ErrorNode,

    #[doc(hidden)]
    __LAST,
}

impl SyntaxKind {
    /// 空白、改行、コメントなどの付随情報(Trivia)であるかを判定する。
    #[inline]
    pub const fn is_trivia(self) -> bool {
        matches!(self, Self::Whitespace | Self::Newline | Self::Comment)
    }

    /// 水平空白または改行であるかを判定する。
    #[inline]
    pub const fn is_whitespace(self) -> bool {
        matches!(self, Self::Whitespace | Self::Newline)
    }

    /// 構文木の葉(終端記号トークン)であるかを判定する。
    #[inline]
    pub const fn is_token(self) -> bool {
        (self as u16) <= (Self::Error as u16)
    }

    /// 構文木の節(非終端記号・複合ノード)であるかを判定する。
    #[inline]
    pub const fn is_node(self) -> bool {
        (self as u16) >= (Self::Root as u16) && (self as u16) < (Self::__LAST as u16)
    }

    /// 不透明ブロックまたはエラーノードであるかを判定する。
    #[inline]
    pub const fn is_opaque_or_error(self) -> bool {
        matches!(
            self,
            Self::OpaqueBlock
                | Self::OpaqueMacro
                | Self::OpaqueEnv
                | Self::TikzPicture
                | Self::ErrorNode
        )
    }

    /// 生の `rowan::SyntaxKind` から `SyntaxKind` を復元する。
    ///
    /// 有効範囲外の値が渡された場合はパニックする。
    #[inline]
    pub fn from_raw(raw: rowan::SyntaxKind) -> Self {
        assert!(
            raw.0 < Self::__LAST as u16,
            "未知の SyntaxKind 生値: {}",
            raw.0
        );
        // SAFETY: 列挙型は連続した 0..__LAST の値を取り、repr(u16) であるため安全。
        unsafe { std::mem::transmute(raw.0) }
    }
}

impl From<SyntaxKind> for rowan::SyntaxKind {
    #[inline]
    fn from(kind: SyntaxKind) -> Self {
        Self(kind as u16)
    }
}

/// `rowan` における LaTeX 言語の定義型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LatexLanguage {}

impl rowan::Language for LatexLanguage {
    type Kind = SyntaxKind;

    #[inline]
    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        SyntaxKind::from_raw(raw)
    }

    #[inline]
    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

/// LaTeX 構文ツリーのノード型。
pub type SyntaxNode = rowan::SyntaxNode<LatexLanguage>;
/// LaTeX 構文ツリーのトークン型。
pub type SyntaxToken = rowan::SyntaxToken<LatexLanguage>;
/// LaTeX 構文ツリーの要素型(ノードまたはトークン)。
pub type SyntaxElement = rowan::SyntaxElement<LatexLanguage>;
/// LaTeX 構文ツリーのノード参照型。
pub type SyntaxNodePtr = rowan::ast::SyntaxNodePtr<LatexLanguage>;
