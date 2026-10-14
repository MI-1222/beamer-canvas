//! `CST`(`GreenNode`/`SyntaxNode`)の編集およびアウトライン操作モジュール。

pub mod error;
pub mod frame_edit;
pub mod handle;
pub mod text_edit;

pub use error::EditError;
pub use frame_edit::FrameEditor;
pub use handle::{FrameHandle, SectionHandle};
pub use text_edit::TextEdit;

use crate::parser::{BeamerParser, FrameNode, ParseResult, SectionNode};
use crate::syntax::{LatexLanguage, SyntaxKind, SyntaxNode};
use rowan::ast::SyntaxNodePtr;

use std::fmt;

/// Beamerドキュメントの構文木に対するアウトライン抽出および編集機能を提供するエディタ。
#[derive(Debug, Clone)]
pub struct DocumentEditor {
    /// ドキュメント全体のルート構文ノード。
    syntax: SyntaxNode,
}

impl fmt::Display for DocumentEditor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.syntax.text())
    }
}

impl DocumentEditor {
    /// 構文解析結果 `ParseResult` から `DocumentEditor` を生成する。
    pub fn new(result: &ParseResult) -> Self {
        Self {
            syntax: result.syntax().clone(),
        }
    }

    /// ルート構文ノード `SyntaxNode` から直接 `DocumentEditor` を生成する。
    pub fn from_syntax(syntax: SyntaxNode) -> Self {
        Self { syntax }
    }

    /// ルート構文ノードへの参照を取得する。
    #[inline]
    pub fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }

    /// ドキュメント全体のソーステキスト表現を取得する。
    #[inline]
    pub fn text(&self) -> String {
        self.syntax.text().to_string()
    }

    /// 本文領域ノード `SyntaxKind::DocumentBody` を取得する。
    pub fn document_body(&self) -> Option<SyntaxNode> {
        self.syntax
            .children()
            .find(|node| node.kind() == SyntaxKind::DocumentBody)
    }

    /// ドキュメント内のスライドフレーム一覧を取得する。
    ///
    /// フレーム内に `SyntaxKind::ErrorNode` が含まれる場合、安全のため `Err(EditError::SyntaxError)` を返す。
    /// エラーを含むフレームを無視して健全なフレームのみを取得したい場合は `get_valid_frames()` を使用する。
    pub fn get_frames(&self) -> Result<Vec<FrameHandle>, EditError> {
        let (frames, _) = self.collect_frames_internal(false)?;
        Ok(frames)
    }

    /// 構文エラーを含むスライドをスキップし、健全なスライドのみを抽出する。
    pub fn get_valid_frames(&self) -> Result<Vec<FrameHandle>, EditError> {
        let (frames, _) = self.collect_frames_internal(true)?;
        Ok(frames)
    }

    /// 構文エラーの有無に関わらず、すべてのスライドを抽出する(各ハンドルにエラーフラグが付与される)。
    pub fn get_all_frames_unchecked(&self) -> Result<Vec<FrameHandle>, EditError> {
        let (frames, _) = self.collect_frames_internal_unchecked()?;
        Ok(frames)
    }

    /// 指定された通し番号(0 開始)のスライドフレームを取得する。
    pub fn get_frame(&self, index: usize) -> Result<FrameHandle, EditError> {
        let frames = self.get_frames()?;
        frames
            .into_iter()
            .find(|f| f.index() == index)
            .ok_or_else(|| EditError::IndexOutOfBounds {
                index,
                max: self.frame_count().unwrap_or(0),
            })
    }

    /// ドキュメント内のスライド総数を取得する。
    pub fn frame_count(&self) -> Result<usize, EditError> {
        Ok(self.get_frames()?.len())
    }

    /// ドキュメント内のセクションおよびサブセクション一覧を取得する。
    ///
    /// 各セクションには、その配下に属するスライド通し番号のリスト(`frame_indices`)が紐付けられる。
    pub fn get_sections(&self) -> Result<Vec<SectionHandle>, EditError> {
        let (_, sections) = self.collect_frames_internal(false)?;
        Ok(sections)
    }

    /// 指定された通し番号(0 開始)のセクションを取得する。
    pub fn get_section(&self, index: usize) -> Result<SectionHandle, EditError> {
        let sections = self.get_sections()?;
        sections
            .into_iter()
            .find(|s| s.index() == index)
            .ok_or_else(|| EditError::IndexOutOfBounds {
                index,
                max: self.section_count().unwrap_or(0),
            })
    }

    /// ドキュメント内のセクション総数を取得する。
    pub fn section_count(&self) -> Result<usize, EditError> {
        Ok(self.get_sections()?.len())
    }

    /// 指定された通し番号のスライドを別の位置へ移動する。
    ///
    /// スライド間に存在するセクションやコメントの位置はそのまま保持する。
    pub fn move_frame(
        &self,
        from_index: usize,
        to_index: usize,
    ) -> Result<DocumentEditor, EditError> {
        FrameEditor::move_frame(self, from_index, to_index)
    }

    /// 指定された2つのスライド位置を相互に入れ替える。
    ///
    /// スライド間に存在するセクションやコメントの位置はそのまま保持する。
    pub fn swap_frames(&self, index_a: usize, index_b: usize) -> Result<DocumentEditor, EditError> {
        FrameEditor::swap_frames(self, index_a, index_b)
    }

    /// 指定された通し番号のスライドを削除する。
    pub fn delete_frame(&self, index: usize) -> Result<DocumentEditor, EditError> {
        FrameEditor::delete_frame(self, index)
    }

    /// 指定された位置に新しいスライドを挿入する。
    ///
    /// `index == frame_count` の場合は末尾に追加する。
    pub fn insert_frame(
        &self,
        index: usize,
        frame_source: &str,
    ) -> Result<DocumentEditor, EditError> {
        FrameEditor::insert_frame(self, index, frame_source)
    }

    /// ドキュメントの末尾に新しいスライドを追加する。
    pub fn push_frame(&self, frame_source: &str) -> Result<DocumentEditor, EditError> {
        let count = self.frame_count()?;
        self.insert_frame(count, frame_source)
    }

    /// 既存のスライド群から代表的な行頭インデント文字列を検出する。
    pub fn detect_indent(&self) -> String {
        let frames = self.get_frames().unwrap_or_default();
        FrameEditor::detect_indent(&frames)
    }

    /// ドキュメントのインデント傾向に合わせて新しいスライドを指定位置に挿入する。
    pub fn insert_frame_with_auto_indent(
        &self,
        index: usize,
        frame_source: &str,
    ) -> Result<DocumentEditor, EditError> {
        let indent = self.detect_indent();
        let formatted = FrameEditor::apply_indent(frame_source, &indent);
        self.insert_frame(index, &formatted)
    }

    /// ドキュメントのインデント傾向に合わせて新しいスライドを末尾に追加する。
    pub fn push_frame_with_auto_indent(
        &self,
        frame_source: &str,
    ) -> Result<DocumentEditor, EditError> {
        let count = self.frame_count()?;
        self.insert_frame_with_auto_indent(count, frame_source)
    }

    /// 現在のエディタ状態から新しい構文解析結果 `ParseResult` を生成する。
    pub fn to_parse_result(&self) -> ParseResult {
        BeamerParser::new().parse(&self.text())
    }

    /// フレームとセクションの走査および関連付けを行う内部ヘルパー。
    ///
    /// `skip_error_frames` が `true` の場合はエラーフレームを除外して処理を継続する。
    /// `false` の場合は最初に見つかったエラーフレームで `Err` を返す。
    fn collect_frames_internal(
        &self,
        skip_error_frames: bool,
    ) -> Result<(Vec<FrameHandle>, Vec<SectionHandle>), EditError> {
        let body = self
            .document_body()
            .ok_or(EditError::DocumentBodyNotFound)?;

        let mut frames = Vec::new();
        let mut sections: Vec<SectionHandle> = Vec::new();

        let mut current_section_idx: Option<usize> = None;
        let mut current_section_ptr: Option<SyntaxNodePtr<LatexLanguage>> = None;
        let mut current_subsection_idx: Option<usize> = None;
        let mut current_subsection_ptr: Option<SyntaxNodePtr<LatexLanguage>> = None;

        let mut frame_seq: usize = 0;
        let mut section_seq: usize = 0;

        for (child_idx, child) in body.children().enumerate() {
            match child.kind() {
                SyntaxKind::Section => {
                    let section_node = match SectionNode::new(&child) {
                        Some(node) => node,
                        None => continue,
                    };
                    let ptr = SyntaxNodePtr::new(&child);
                    let sec_idx = section_seq;
                    section_seq += 1;

                    current_section_idx = Some(sec_idx);
                    current_section_ptr = Some(ptr);
                    current_subsection_idx = None;
                    current_subsection_ptr = None;

                    sections.push(SectionHandle {
                        index: sec_idx,
                        is_subsection: false,
                        ptr,
                        node: section_node,
                        parent_section_index: None,
                        frame_indices: Vec::new(),
                        child_index_in_body: child_idx,
                    });
                }
                SyntaxKind::Subsection => {
                    let section_node = match SectionNode::new(&child) {
                        Some(node) => node,
                        None => continue,
                    };
                    let ptr = SyntaxNodePtr::new(&child);
                    let subsec_idx = section_seq;
                    section_seq += 1;

                    current_subsection_idx = Some(subsec_idx);
                    current_subsection_ptr = Some(ptr);

                    sections.push(SectionHandle {
                        index: subsec_idx,
                        is_subsection: true,
                        ptr,
                        node: section_node,
                        parent_section_index: current_section_idx,
                        frame_indices: Vec::new(),
                        child_index_in_body: child_idx,
                    });
                }
                SyntaxKind::Frame => {
                    let frame_node = match FrameNode::new(&child) {
                        Some(node) => node,
                        None => continue,
                    };
                    let has_errors = child
                        .descendants()
                        .any(|node| node.kind() == SyntaxKind::ErrorNode);

                    if has_errors {
                        if !skip_error_frames {
                            return Err(EditError::SyntaxError {
                                message: "スライドフレーム内に構文エラーが存在する。".to_string(),
                                index: Some(frame_seq),
                            });
                        }
                        // スキップモードの場合はエラーフレームを無視して継続する。
                        continue;
                    }

                    let ptr = SyntaxNodePtr::new(&child);
                    let this_frame_idx = frame_seq;
                    frame_seq += 1;

                    // 所属する直近のセクションまたはサブセクションの `frame_indices` に追加する。
                    if let Some(sec_idx) = current_section_idx
                        && let Some(sec) = sections.iter_mut().find(|s| s.index == sec_idx)
                    {
                        sec.frame_indices.push(this_frame_idx);
                    }
                    if let Some(subsec_idx) = current_subsection_idx
                        && let Some(subsec) = sections.iter_mut().find(|s| s.index == subsec_idx)
                    {
                        subsec.frame_indices.push(this_frame_idx);
                    }

                    frames.push(FrameHandle {
                        index: this_frame_idx,
                        ptr,
                        node: frame_node,
                        section_index: current_section_idx,
                        section_ptr: current_section_ptr,
                        subsection_index: current_subsection_idx,
                        subsection_ptr: current_subsection_ptr,
                        child_index_in_body: child_idx,
                        has_errors: false,
                    });
                }
                _ => {}
            }
        }

        Ok((frames, sections))
    }

    /// エラーチェックを行わずにすべてのフレームおよびセクションを収集する内部ヘルパー。
    fn collect_frames_internal_unchecked(
        &self,
    ) -> Result<(Vec<FrameHandle>, Vec<SectionHandle>), EditError> {
        let body = self
            .document_body()
            .ok_or(EditError::DocumentBodyNotFound)?;

        let mut frames = Vec::new();
        let mut sections: Vec<SectionHandle> = Vec::new();

        let mut current_section_idx: Option<usize> = None;
        let mut current_section_ptr: Option<SyntaxNodePtr<LatexLanguage>> = None;
        let mut current_subsection_idx: Option<usize> = None;
        let mut current_subsection_ptr: Option<SyntaxNodePtr<LatexLanguage>> = None;

        let mut frame_seq: usize = 0;
        let mut section_seq: usize = 0;

        for (child_idx, child) in body.children().enumerate() {
            match child.kind() {
                SyntaxKind::Section => {
                    let section_node = match SectionNode::new(&child) {
                        Some(node) => node,
                        None => continue,
                    };
                    let ptr = SyntaxNodePtr::new(&child);
                    let sec_idx = section_seq;
                    section_seq += 1;

                    current_section_idx = Some(sec_idx);
                    current_section_ptr = Some(ptr);
                    current_subsection_idx = None;
                    current_subsection_ptr = None;

                    sections.push(SectionHandle {
                        index: sec_idx,
                        is_subsection: false,
                        ptr,
                        node: section_node,
                        parent_section_index: None,
                        frame_indices: Vec::new(),
                        child_index_in_body: child_idx,
                    });
                }
                SyntaxKind::Subsection => {
                    let section_node = match SectionNode::new(&child) {
                        Some(node) => node,
                        None => continue,
                    };
                    let ptr = SyntaxNodePtr::new(&child);
                    let subsec_idx = section_seq;
                    section_seq += 1;

                    current_subsection_idx = Some(subsec_idx);
                    current_subsection_ptr = Some(ptr);

                    sections.push(SectionHandle {
                        index: subsec_idx,
                        is_subsection: true,
                        ptr,
                        node: section_node,
                        parent_section_index: current_section_idx,
                        frame_indices: Vec::new(),
                        child_index_in_body: child_idx,
                    });
                }
                SyntaxKind::Frame => {
                    let frame_node = match FrameNode::new(&child) {
                        Some(node) => node,
                        None => continue,
                    };
                    let has_errors = child
                        .descendants()
                        .any(|node| node.kind() == SyntaxKind::ErrorNode);

                    let ptr = SyntaxNodePtr::new(&child);
                    let this_frame_idx = frame_seq;
                    frame_seq += 1;

                    if let Some(sec_idx) = current_section_idx
                        && let Some(sec) = sections.iter_mut().find(|s| s.index == sec_idx)
                    {
                        sec.frame_indices.push(this_frame_idx);
                    }
                    if let Some(subsec_idx) = current_subsection_idx
                        && let Some(subsec) = sections.iter_mut().find(|s| s.index == subsec_idx)
                    {
                        subsec.frame_indices.push(this_frame_idx);
                    }

                    frames.push(FrameHandle {
                        index: this_frame_idx,
                        ptr,
                        node: frame_node,
                        section_index: current_section_idx,
                        section_ptr: current_section_ptr,
                        subsection_index: current_subsection_idx,
                        subsection_ptr: current_subsection_ptr,
                        child_index_in_body: child_idx,
                        has_errors,
                    });
                }
                _ => {}
            }
        }

        Ok((frames, sections))
    }
}
