//! スライドおよびセクションを追跡・操作するためのハンドル型モジュール。

use crate::parser::{
    BlockNode, ColumnsNode, FrameNode, IncludeGraphicsNode, ListNode, PauseNode, SectionNode,
};
use crate::syntax::{LatexLanguage, SyntaxNode};
use rowan::ast::SyntaxNodePtr;

/// ドキュメント内のスライドフレームを特定・操作するためのハンドル型。
#[derive(Debug, Clone)]
pub struct FrameHandle {
    /// ドキュメント全体におけるスライド通し番号(0 開始)。
    pub(crate) index: usize,
    /// 構文木上のノードを追跡するためのポインタ。
    pub(crate) ptr: SyntaxNodePtr<LatexLanguage>,
    /// ラップされた `FrameNode` インスタンス。
    pub(crate) node: FrameNode,
    /// 所属する親セクションのインデックス。
    pub(crate) section_index: Option<usize>,
    /// 所属する親セクションノードのポインタ。
    pub(crate) section_ptr: Option<SyntaxNodePtr<LatexLanguage>>,
    /// 所属する親サブセクションのインデックス。
    pub(crate) subsection_index: Option<usize>,
    /// 所属する親サブセクションノードのポインタ。
    pub(crate) subsection_ptr: Option<SyntaxNodePtr<LatexLanguage>>,
    /// `DocumentBody` 直下の子ノードとしてのインデックス位置。
    pub(crate) child_index_in_body: usize,
    /// フレーム配下に構文エラーノード `ErrorNode` が含まれているかのフラグ。
    pub(crate) has_errors: bool,
}

impl FrameHandle {
    /// スライド通し番号(0 開始)を取得する。
    #[inline]
    pub fn index(&self) -> usize {
        self.index
    }

    /// 表示用のスライド番号(1 開始)を取得する。
    #[inline]
    pub fn slide_number(&self) -> usize {
        self.index + 1
    }

    /// 構文木上のノードを指す `SyntaxNodePtr` を取得する。
    #[inline]
    pub fn ptr(&self) -> SyntaxNodePtr<LatexLanguage> {
        self.ptr
    }

    /// 基底の `FrameNode` への参照を取得する。
    #[inline]
    pub fn node(&self) -> &FrameNode {
        &self.node
    }

    /// 基底の `SyntaxNode` への参照を取得する。
    #[inline]
    pub fn syntax(&self) -> &SyntaxNode {
        self.node.syntax()
    }

    /// 所属する親セクションのインデックスを取得する。
    #[inline]
    pub fn section_index(&self) -> Option<usize> {
        self.section_index
    }

    /// 所属する親セクションノードのポインタを取得する。
    #[inline]
    pub fn section_ptr(&self) -> Option<SyntaxNodePtr<LatexLanguage>> {
        self.section_ptr
    }

    /// 所属する親サブセクションのインデックスを取得する。
    #[inline]
    pub fn subsection_index(&self) -> Option<usize> {
        self.subsection_index
    }

    /// 所属する親サブセクションノードのポインタを取得する。
    #[inline]
    pub fn subsection_ptr(&self) -> Option<SyntaxNodePtr<LatexLanguage>> {
        self.subsection_ptr
    }

    /// `DocumentBody` 直下の子ノードとしてのインデックス位置を取得する。
    #[inline]
    pub fn child_index_in_body(&self) -> usize {
        self.child_index_in_body
    }

    /// フレーム配下に構文エラーノード `ErrorNode` が含まれているかを判定する。
    #[inline]
    pub fn has_errors(&self) -> bool {
        self.has_errors
    }

    /// スライドタイトルの文字列を取得する。
    #[inline]
    pub fn title_text(&self) -> Option<String> {
        self.node.title_text()
    }

    /// スライドサブタイトルの文字列を取得する。
    #[inline]
    pub fn subtitle_text(&self) -> Option<String> {
        self.node.subtitle_text()
    }

    /// オプション引数の文字列を取得する。
    #[inline]
    pub fn options_text(&self) -> Option<String> {
        self.node.options_text()
    }

    /// オーバーレイスペックの文字列を取得する。
    #[inline]
    pub fn overlay_spec_text(&self) -> Option<String> {
        self.node.overlay_spec_text()
    }

    /// スライド内のブロック要素一覧を取得する。
    #[inline]
    pub fn blocks(&self) -> Vec<BlockNode> {
        self.node.blocks()
    }

    /// スライド内の段組みレイアウト一覧を取得する。
    #[inline]
    pub fn columns_layouts(&self) -> Vec<ColumnsNode> {
        self.node.columns_layouts()
    }

    /// スライド内の箇条書き環境一覧を取得する。
    #[inline]
    pub fn lists(&self) -> Vec<ListNode> {
        self.node.lists()
    }

    /// スライド内の画像挿入要素一覧を取得する。
    #[inline]
    pub fn images(&self) -> Vec<IncludeGraphicsNode> {
        self.node.images()
    }

    /// スライド内の `\pause` コマンド一覧を取得する。
    #[inline]
    pub fn pause_commands(&self) -> Vec<PauseNode> {
        self.node.pause_commands()
    }

    /// 当該スライドのタイトルを更新する。
    pub fn set_title(
        &self,
        editor: &crate::edit::DocumentEditor,
        title: &str,
    ) -> Result<crate::edit::DocumentEditor, crate::edit::EditError> {
        editor.set_frame_title(self.index, title)
    }

    /// 当該スライドのサブタイトルを更新する。
    pub fn set_subtitle(
        &self,
        editor: &crate::edit::DocumentEditor,
        subtitle: &str,
    ) -> Result<crate::edit::DocumentEditor, crate::edit::EditError> {
        editor.set_frame_subtitle(self.index, subtitle)
    }

    /// 当該スライドのオプション引数を更新する。
    pub fn set_options(
        &self,
        editor: &crate::edit::DocumentEditor,
        options: &str,
    ) -> Result<crate::edit::DocumentEditor, crate::edit::EditError> {
        editor.set_frame_options(self.index, options)
    }

    /// 当該スライド内の指定画像の幅を更新する。
    pub fn set_image_width(
        &self,
        editor: &crate::edit::DocumentEditor,
        image_index: usize,
        width: &str,
    ) -> Result<crate::edit::DocumentEditor, crate::edit::EditError> {
        editor.set_image_width(self.index, image_index, width)
    }
}

/// ドキュメント内のセクション境界を特定・操作するためのハンドル型。
#[derive(Debug, Clone)]
pub struct SectionHandle {
    /// セクション通し番号(0 開始)。
    pub(crate) index: usize,
    /// サブセクションであるかのフラグ。
    pub(crate) is_subsection: bool,
    /// 構文木上のノードを追跡するためのポインタ。
    pub(crate) ptr: SyntaxNodePtr<LatexLanguage>,
    /// ラップされた `SectionNode` インスタンス。
    pub(crate) node: SectionNode,
    /// 親セクション(`\section`)のインデックス(自身がサブセクションの場合)。
    pub(crate) parent_section_index: Option<usize>,
    /// このセクション配下に属するスライド通し番号のリスト。
    pub(crate) frame_indices: Vec<usize>,
    /// `DocumentBody` 直下の子ノードとしてのインデックス位置。
    pub(crate) child_index_in_body: usize,
}

impl SectionHandle {
    /// セクション通し番号(0 開始)を取得する。
    #[inline]
    pub fn index(&self) -> usize {
        self.index
    }

    /// サブセクション(`\subsection`)であるかを判定する。
    #[inline]
    pub fn is_subsection(&self) -> bool {
        self.is_subsection
    }

    /// 構文木上のノードを指す `SyntaxNodePtr` を取得する。
    #[inline]
    pub fn ptr(&self) -> SyntaxNodePtr<LatexLanguage> {
        self.ptr
    }

    /// 基底の `SectionNode` への参照を取得する。
    #[inline]
    pub fn node(&self) -> &SectionNode {
        &self.node
    }

    /// 基底の `SyntaxNode` への参照を取得する。
    #[inline]
    pub fn syntax(&self) -> &SyntaxNode {
        self.node.syntax()
    }

    /// 親セクション(`\section`)のインデックスを取得する。
    #[inline]
    pub fn parent_section_index(&self) -> Option<usize> {
        self.parent_section_index
    }

    /// このセクション配下に属するスライド通し番号一覧を取得する。
    #[inline]
    pub fn frame_indices(&self) -> &[usize] {
        &self.frame_indices
    }

    /// `DocumentBody` 直下の子ノードとしてのインデックス位置を取得する。
    #[inline]
    pub fn child_index_in_body(&self) -> usize {
        self.child_index_in_body
    }

    /// セクションタイトルの文字列を取得する。
    #[inline]
    pub fn title_text(&self) -> Option<String> {
        self.node.title_text()
    }

    /// オプション引数の文字列を取得する。
    #[inline]
    pub fn options_text(&self) -> Option<String> {
        self.node.options_text()
    }
}
