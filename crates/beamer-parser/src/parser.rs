//! 構文解析器および `GreenNode` 構築モジュール。

use crate::grammar::ParserContext;
use crate::grammar::frame::{parse_frame_env, parse_legacy_frame};
use crate::grammar::section::parse_section;
use crate::lexer::tokenize;
use crate::syntax::{SyntaxKind, SyntaxNode};
use crate::token_source::TokenSource;
use rowan::GreenNodeBuilder;

/// Beamer ソースコードの構文解析結果を保持する構造体。
#[derive(Debug, Clone)]
pub struct ParseResult {
    /// 構築されたルート構文ノード。
    pub syntax: SyntaxNode,
    /// 構文解析中に検出されたエラーのリスト。
    pub errors: Vec<String>,
}

impl ParseResult {
    /// ルート構文ノード `SyntaxKind::Root` の参照を取得する。
    #[inline]
    pub fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }

    /// プリアンブル領域ノード `SyntaxKind::Preamble` を探索して取得する。
    pub fn preamble(&self) -> Option<SyntaxNode> {
        self.syntax
            .children()
            .find(|node| node.kind() == SyntaxKind::Preamble)
    }

    /// 本文領域ノード `SyntaxKind::DocumentBody` を探索して取得する。
    pub fn document_body(&self) -> Option<SyntaxNode> {
        self.syntax
            .children()
            .find(|node| node.kind() == SyntaxKind::DocumentBody)
    }

    /// 本文に含まれるすべてのスライドフレームノード `SyntaxKind::Frame` を取得する。
    pub fn frames(&self) -> Vec<SyntaxNode> {
        let body = match self.document_body() {
            Some(body) => body,
            None => return Vec::new(),
        };
        body.children()
            .filter(|node| node.kind() == SyntaxKind::Frame)
            .collect()
    }

    /// 本文に含まれるすべてのセクションおよびサブセクションノードを取得する。
    pub fn sections(&self) -> Vec<SyntaxNode> {
        let body = match self.document_body() {
            Some(body) => body,
            None => return Vec::new(),
        };
        body.children()
            .filter(|node| matches!(node.kind(), SyntaxKind::Section | SyntaxKind::Subsection))
            .collect()
    }
}

/// ソースコードから具象構文木 `CST` を構築するパーサー構造体。
#[derive(Debug, Default, Clone, Copy)]
pub struct BeamerParser;

impl BeamerParser {
    /// 新しい `BeamerParser` インスタンスを生成する。
    pub fn new() -> Self {
        Self
    }

    /// LaTeX Beamer ソース文字列を解析し、ロスレスな構文木を構築する。
    pub fn parse(&self, text: &str) -> ParseResult {
        let tokens = tokenize(text);
        let mut source = TokenSource::new(tokens);
        let mut builder = GreenNodeBuilder::new();
        let mut errors = Vec::new();

        builder.start_node(SyntaxKind::Root.into());

        if !source.is_eof() {
            let has_document_env = has_env_begin_in_stream(&source, "document");

            if has_document_env {
                // 1. `\begin{document}` 直前までのプリアンブル領域を解析する。
                builder.start_node(SyntaxKind::Preamble.into());
                while !source.is_eof() && !source.is_at_env_begin_boundary("document") {
                    let token = source.bump().expect("EOF ではない。");
                    builder.token(token.kind.into(), token.text);
                }
                builder.finish_node();

                // 2. `\begin{document}` から `\end{document}` までの本文領域を解析する。
                if source.is_at_env_begin_boundary("document") {
                    builder.start_node(SyntaxKind::DocumentBody.into());

                    // `\begin{document}` タグを消費する。
                    let begin_end_idx = source
                        .env_tag_end_index()
                        .expect("環境タグ終了インデックスが存在する。");
                    while source.cursor() <= begin_end_idx {
                        let token = source.bump().expect("タグ範囲内のトークンが存在する。");
                        builder.token(token.kind.into(), token.text);
                    }

                    // 本文内の `Frame` や `Section` などの各構造要素を解析する。
                    let mut ctx = ParserContext::new(&mut source, &mut builder, &mut errors);
                    while !ctx.source.is_eof() && !ctx.source.is_at_env_end_boundary("document") {
                        let prev_cursor = ctx.source.cursor();

                        if ctx.source.is_at_env_begin_boundary("frame") {
                            parse_frame_env(&mut ctx);
                        } else if ctx.source.is_at_command_boundary("\\frame") {
                            parse_legacy_frame(&mut ctx);
                        } else if ctx.source.is_at_command_boundary("\\section") {
                            parse_section(&mut ctx, false);
                        } else if ctx.source.is_at_command_boundary("\\subsection") {
                            parse_section(&mut ctx, true);
                        } else {
                            ctx.bump();
                        }

                        // 各反復でトークンが進んでいることを保証し、無限ループを防ぐ。
                        if ctx.source.cursor() == prev_cursor && !ctx.source.is_eof() {
                            ctx.bump();
                        }
                    }

                    // `\end{document}` タグが存在する場合に消費する。
                    if source.is_at_env_end_boundary("document") {
                        let end_idx = source
                            .env_tag_end_index()
                            .expect("環境タグ終了インデックスが存在する。");
                        while source.cursor() <= end_idx {
                            let token = source.bump().expect("タグ範囲内のトークンが存在する。");
                            builder.token(token.kind.into(), token.text);
                        }
                    }

                    builder.finish_node();
                }

                // 3. `\end{document}` 以降の後続トークンを `Root` 直下に消費する。
                while !source.is_eof() {
                    let token = source.bump().expect("EOF ではない。");
                    builder.token(token.kind.into(), token.text);
                }
            } else {
                // `\begin{document}` が存在しない場合は全体を `Preamble` として保持する。
                builder.start_node(SyntaxKind::Preamble.into());
                while !source.is_eof() {
                    let token = source.bump().expect("EOF ではない。");
                    builder.token(token.kind.into(), token.text);
                }
                builder.finish_node();
            }
        }

        builder.finish_node();

        let green_node = builder.finish();
        let syntax = SyntaxNode::new_root(green_node);

        ParseResult { syntax, errors }
    }
}

/// トークンストリーム内に指定環境の開始タグが存在するかを走査して判定する。
fn has_env_begin_in_stream(source: &TokenSource<'_>, env_name: &str) -> bool {
    let mut check_source = source.clone();
    while !check_source.is_eof() {
        if check_source.is_at_env_begin(env_name) {
            return true;
        }
        check_source.bump();
    }
    false
}

/// スライドフレームノード `SyntaxKind::Frame` の要素参照を提供するヘルパー型。
#[derive(Debug, Clone)]
pub struct FrameNode {
    syntax: SyntaxNode,
}

impl FrameNode {
    /// `SyntaxKind::Frame` ノードから `FrameNode` ラッパーを生成する。
    pub fn new(syntax: &SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::Frame {
            Some(Self {
                syntax: syntax.clone(),
            })
        } else {
            None
        }
    }

    /// 基底の `SyntaxNode` への参照を取得する。
    #[inline]
    pub fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }

    /// フレームヘッダーノード `SyntaxKind::FrameHeader` を取得する。
    pub fn header(&self) -> Option<SyntaxNode> {
        self.syntax
            .children()
            .find(|n| n.kind() == SyntaxKind::FrameHeader)
    }

    /// 角括弧オプションノード `SyntaxKind::FrameOptions` を取得する。
    pub fn options(&self) -> Option<SyntaxNode> {
        self.header()?
            .children()
            .find(|n| n.kind() == SyntaxKind::FrameOptions)
    }

    /// フレームタイトル引数ノード `SyntaxKind::FrameTitle` を取得する。
    pub fn title_node(&self) -> Option<SyntaxNode> {
        self.header()?
            .children()
            .find(|n| n.kind() == SyntaxKind::FrameTitle)
    }

    /// フレームサブタイトル引数ノード `SyntaxKind::FrameSubtitle` を取得する。
    pub fn subtitle_node(&self) -> Option<SyntaxNode> {
        self.header()?
            .children()
            .find(|n| n.kind() == SyntaxKind::FrameSubtitle)
    }

    /// オーバーレイスペックノード `SyntaxKind::OverlaySpec` を取得する。
    pub fn overlay_spec(&self) -> Option<SyntaxNode> {
        self.header()?
            .children()
            .find(|n| n.kind() == SyntaxKind::OverlaySpec)
    }

    /// 角括弧を除いたオプション引数の内部テキストを取得する。
    pub fn options_text(&self) -> Option<String> {
        let options = self.options()?;
        let text = options.text().to_string();
        let trimmed = text.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2 {
            Some(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            Some(trimmed.to_string())
        }
    }

    /// 波括弧を除いたタイトル引数の内部テキストを取得する。
    ///
    /// ヘッダー引数に存在しない場合は、フレーム本文内の `\frametitle{...}` からの取得を試みる。
    pub fn title_text(&self) -> Option<String> {
        if let Some(title_node) = self.title_node() {
            let text = title_node.text().to_string();
            let trimmed = text.trim();
            if trimmed.starts_with('{') && trimmed.ends_with('}') && trimmed.len() >= 2 {
                return Some(trimmed[1..trimmed.len() - 1].to_string());
            }
            return Some(trimmed.to_string());
        }

        // ヘッダー引数にタイトルがない場合は、フレーム直下の `\frametitle{...}` を探索する。
        find_frametitle_in_node(&self.syntax)
    }

    /// 波括弧を除いたサブタイトル引数の内部テキストを取得する。
    pub fn subtitle_text(&self) -> Option<String> {
        let subtitle_node = self.subtitle_node()?;
        let text = subtitle_node.text().to_string();
        let trimmed = text.trim();
        if trimmed.starts_with('{') && trimmed.ends_with('}') && trimmed.len() >= 2 {
            Some(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            Some(trimmed.to_string())
        }
    }

    /// 山括弧を除いたオーバーレイスペックの内部テキストを取得する。
    pub fn overlay_spec_text(&self) -> Option<String> {
        let spec_node = self.overlay_spec()?;
        let text = spec_node.text().to_string();
        let trimmed = text.trim();
        if trimmed.starts_with('<') && trimmed.ends_with('>') && trimmed.len() >= 2 {
            Some(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            Some(trimmed.to_string())
        }
    }

    /// フレーム配下のブロックノード `SyntaxKind::Block` をすべて取得する。
    pub fn blocks(&self) -> Vec<BlockNode> {
        self.syntax
            .descendants()
            .filter_map(|n| BlockNode::new(&n))
            .collect()
    }

    /// フレーム配下の段組みノード `SyntaxKind::Columns` をすべて取得する。
    pub fn columns_layouts(&self) -> Vec<ColumnsNode> {
        self.syntax
            .descendants()
            .filter_map(|n| ColumnsNode::new(&n))
            .collect()
    }

    /// フレーム配下の箇条書き環境ノードをすべて取得する。
    pub fn lists(&self) -> Vec<ListNode> {
        self.syntax
            .descendants()
            .filter_map(|n| ListNode::new(&n))
            .collect()
    }

    /// フレーム配下の画像挿入ノード `SyntaxKind::IncludeGraphics` をすべて取得する。
    pub fn images(&self) -> Vec<IncludeGraphicsNode> {
        self.syntax
            .descendants()
            .filter_map(|n| IncludeGraphicsNode::new(&n))
            .collect()
    }

    /// フレーム配下の `\pause` コマンドノード `SyntaxKind::Pause` をすべて取得する。
    pub fn pause_commands(&self) -> Vec<PauseNode> {
        self.syntax
            .descendants()
            .filter_map(|n| PauseNode::new(&n))
            .collect()
    }
}

/// ブロック環境ノード `SyntaxKind::Block` をラップする型。
#[derive(Debug, Clone)]
pub struct BlockNode {
    syntax: SyntaxNode,
}

impl BlockNode {
    /// `SyntaxKind::Block` ノードからラッパーを生成する。
    pub fn new(syntax: &SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::Block {
            Some(Self {
                syntax: syntax.clone(),
            })
        } else {
            None
        }
    }

    /// 基底の `SyntaxNode` への参照を取得する。
    #[inline]
    pub fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }

    /// 波括弧を除いたタイトル引数の内部テキストを取得する。
    pub fn title_text(&self) -> Option<String> {
        let arg = self
            .syntax
            .children()
            .find(|n| n.kind() == SyntaxKind::Argument)?;
        let text = arg.text().to_string();
        let trimmed = text.trim();
        if trimmed.starts_with('{') && trimmed.ends_with('}') && trimmed.len() >= 2 {
            Some(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            Some(trimmed.to_string())
        }
    }

    /// 山括弧を除いたオーバーレイスペックの内部テキストを取得する。
    pub fn overlay_spec_text(&self) -> Option<String> {
        let spec = self
            .syntax
            .children()
            .find(|n| n.kind() == SyntaxKind::OverlaySpec)?;
        let text = spec.text().to_string();
        let trimmed = text.trim();
        if trimmed.starts_with('<') && trimmed.ends_with('>') && trimmed.len() >= 2 {
            Some(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            Some(trimmed.to_string())
        }
    }

    /// 角括弧を除いたオプション引数の内部テキストを取得する。
    pub fn options_text(&self) -> Option<String> {
        let opt = self
            .syntax
            .children()
            .find(|n| n.kind() == SyntaxKind::OptionsList)?;
        let text = opt.text().to_string();
        let trimmed = text.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2 {
            Some(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            Some(trimmed.to_string())
        }
    }
}

/// 段組み環境ノード `SyntaxKind::Columns` をラップする型。
#[derive(Debug, Clone)]
pub struct ColumnsNode {
    syntax: SyntaxNode,
}

impl ColumnsNode {
    /// `SyntaxKind::Columns` ノードからラッパーを生成する。
    pub fn new(syntax: &SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::Columns {
            Some(Self {
                syntax: syntax.clone(),
            })
        } else {
            None
        }
    }

    /// 基底の `SyntaxNode` への参照を取得する。
    #[inline]
    pub fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }

    /// 角括弧を除いたオプション引数の内部テキストを取得する。
    pub fn options_text(&self) -> Option<String> {
        let opt = self
            .syntax
            .children()
            .find(|n| n.kind() == SyntaxKind::OptionsList)?;
        let text = opt.text().to_string();
        let trimmed = text.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2 {
            Some(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            Some(trimmed.to_string())
        }
    }

    /// 段組み内部に含まれる列ノード `ColumnNode` の一覧を取得する。
    pub fn columns(&self) -> Vec<ColumnNode> {
        self.syntax
            .children()
            .filter_map(|n| ColumnNode::new(&n))
            .collect()
    }
}

/// 単一列ノード `SyntaxKind::Column` をラップする型。
#[derive(Debug, Clone)]
pub struct ColumnNode {
    syntax: SyntaxNode,
}

impl ColumnNode {
    /// `SyntaxKind::Column` ノードからラッパーを生成する。
    pub fn new(syntax: &SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::Column {
            Some(Self {
                syntax: syntax.clone(),
            })
        } else {
            None
        }
    }

    /// 基底の `SyntaxNode` への参照を取得する。
    #[inline]
    pub fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }

    /// 波括弧を除いた列幅指定の内部テキストを取得する。
    pub fn width_text(&self) -> Option<String> {
        let arg = self
            .syntax
            .children()
            .find(|n| n.kind() == SyntaxKind::Argument)?;
        let text = arg.text().to_string();
        let trimmed = text.trim();
        if trimmed.starts_with('{') && trimmed.ends_with('}') && trimmed.len() >= 2 {
            Some(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            Some(trimmed.to_string())
        }
    }

    /// 角括弧を除いた配置オプションの内部テキストを取得する。
    pub fn placement_text(&self) -> Option<String> {
        let opt = self
            .syntax
            .children()
            .find(|n| n.kind() == SyntaxKind::OptionsList)?;
        let text = opt.text().to_string();
        let trimmed = text.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2 {
            Some(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            Some(trimmed.to_string())
        }
    }
}

/// 箇条書き環境ノード `SyntaxKind::Itemize` または `SyntaxKind::Enumerate` をラップする型。
#[derive(Debug, Clone)]
pub struct ListNode {
    syntax: SyntaxNode,
}

impl ListNode {
    /// リスト環境ノードからラッパーを生成する。
    pub fn new(syntax: &SyntaxNode) -> Option<Self> {
        if matches!(syntax.kind(), SyntaxKind::Itemize | SyntaxKind::Enumerate) {
            Some(Self {
                syntax: syntax.clone(),
            })
        } else {
            None
        }
    }

    /// 基底の `SyntaxNode` への参照を取得する。
    #[inline]
    pub fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }

    /// 番号付きリスト `enumerate` であるかを判定する。
    #[inline]
    pub fn is_enumerate(&self) -> bool {
        self.syntax.kind() == SyntaxKind::Enumerate
    }

    /// リスト内に含まれるすべての項目ノード `ItemNode` を取得する。
    pub fn items(&self) -> Vec<ItemNode> {
        self.syntax
            .children()
            .filter_map(|n| ItemNode::new(&n))
            .collect()
    }
}

/// 箇条書き項目ノード `SyntaxKind::Item` をラップする型。
#[derive(Debug, Clone)]
pub struct ItemNode {
    syntax: SyntaxNode,
}

impl ItemNode {
    /// `SyntaxKind::Item` ノードからラッパーを生成する。
    pub fn new(syntax: &SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::Item {
            Some(Self {
                syntax: syntax.clone(),
            })
        } else {
            None
        }
    }

    /// 基底の `SyntaxNode` への参照を取得する。
    #[inline]
    pub fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }

    /// 山括弧を除いたオーバーレイスペックの内部テキストを取得する。
    pub fn overlay_spec_text(&self) -> Option<String> {
        let spec = self
            .syntax
            .children()
            .find(|n| n.kind() == SyntaxKind::OverlaySpec)?;
        let text = spec.text().to_string();
        let trimmed = text.trim();
        if trimmed.starts_with('<') && trimmed.ends_with('>') && trimmed.len() >= 2 {
            Some(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            Some(trimmed.to_string())
        }
    }

    /// 角括弧を除いたラベル・オプション引数の内部テキストを取得する。
    pub fn options_text(&self) -> Option<String> {
        let opt = self
            .syntax
            .children()
            .find(|n| n.kind() == SyntaxKind::OptionsList)?;
        let text = opt.text().to_string();
        let trimmed = text.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2 {
            Some(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            Some(trimmed.to_string())
        }
    }
}

/// 画像挿入ノード `SyntaxKind::IncludeGraphics` をラップする型。
#[derive(Debug, Clone)]
pub struct IncludeGraphicsNode {
    syntax: SyntaxNode,
}

impl IncludeGraphicsNode {
    /// `SyntaxKind::IncludeGraphics` ノードからラッパーを生成する。
    pub fn new(syntax: &SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::IncludeGraphics {
            Some(Self {
                syntax: syntax.clone(),
            })
        } else {
            None
        }
    }

    /// 基底の `SyntaxNode` への参照を取得する。
    #[inline]
    pub fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }

    /// 波括弧を除いた画像ファイルパスの内部テキストを取得する。
    pub fn path_text(&self) -> Option<String> {
        let arg = self
            .syntax
            .children()
            .find(|n| n.kind() == SyntaxKind::Argument)?;
        let text = arg.text().to_string();
        let trimmed = text.trim();
        if trimmed.starts_with('{') && trimmed.ends_with('}') && trimmed.len() >= 2 {
            Some(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            Some(trimmed.to_string())
        }
    }

    /// 角括弧を除いたオプション引数の内部テキストを取得する。
    pub fn options_text(&self) -> Option<String> {
        let opt = self
            .syntax
            .children()
            .find(|n| n.kind() == SyntaxKind::OptionsList)?;
        let text = opt.text().to_string();
        let trimmed = text.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2 {
            Some(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            Some(trimmed.to_string())
        }
    }

    /// 山括弧を除いたオーバーレイスペックの内部テキストを取得する。
    pub fn overlay_spec_text(&self) -> Option<String> {
        let spec = self
            .syntax
            .children()
            .find(|n| n.kind() == SyntaxKind::OverlaySpec)?;
        let text = spec.text().to_string();
        let trimmed = text.trim();
        if trimmed.starts_with('<') && trimmed.ends_with('>') && trimmed.len() >= 2 {
            Some(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            Some(trimmed.to_string())
        }
    }
}

/// 一時停止コマンドノード `SyntaxKind::Pause` をラップする型。
#[derive(Debug, Clone)]
pub struct PauseNode {
    syntax: SyntaxNode,
}

impl PauseNode {
    /// `SyntaxKind::Pause` ノードからラッパーを生成する。
    pub fn new(syntax: &SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::Pause {
            Some(Self {
                syntax: syntax.clone(),
            })
        } else {
            None
        }
    }

    /// 基底の `SyntaxNode` への参照を取得する。
    #[inline]
    pub fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }

    /// 山括弧を除いたオーバーレイスペックの内部テキストを取得する。
    pub fn overlay_spec_text(&self) -> Option<String> {
        let spec = self
            .syntax
            .children()
            .find(|n| n.kind() == SyntaxKind::OverlaySpec)?;
        let text = spec.text().to_string();
        let trimmed = text.trim();
        if trimmed.starts_with('<') && trimmed.ends_with('>') && trimmed.len() >= 2 {
            Some(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            Some(trimmed.to_string())
        }
    }
}

/// セクション境界ノード `SyntaxKind::Section` または `SyntaxKind::Subsection` の参照を提供するヘルパー型。
#[derive(Debug, Clone)]
pub struct SectionNode {
    syntax: SyntaxNode,
}

impl SectionNode {
    /// `SyntaxKind::Section` または `SyntaxKind::Subsection` ノードからラッパーを生成する。
    pub fn new(syntax: &SyntaxNode) -> Option<Self> {
        if matches!(syntax.kind(), SyntaxKind::Section | SyntaxKind::Subsection) {
            Some(Self {
                syntax: syntax.clone(),
            })
        } else {
            None
        }
    }

    /// 基底の `SyntaxNode` への参照を取得する。
    #[inline]
    pub fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }

    /// サブセクションであるかを判定する。
    #[inline]
    pub fn is_subsection(&self) -> bool {
        self.syntax.kind() == SyntaxKind::Subsection
    }

    /// 波括弧を除いたタイトル引数の内部テキストを取得する。
    pub fn title_text(&self) -> Option<String> {
        let arg = self
            .syntax
            .children()
            .find(|n| n.kind() == SyntaxKind::Argument)?;
        let text = arg.text().to_string();
        let trimmed = text.trim();
        if trimmed.starts_with('{') && trimmed.ends_with('}') && trimmed.len() >= 2 {
            Some(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            Some(trimmed.to_string())
        }
    }

    /// 角括弧を除いたオプション引数の内部テキストを取得する。
    pub fn options_text(&self) -> Option<String> {
        let opt = self
            .syntax
            .children()
            .find(|n| n.kind() == SyntaxKind::OptionsList)?;
        let text = opt.text().to_string();
        let trimmed = text.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2 {
            Some(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            Some(trimmed.to_string())
        }
    }
}

/// ノード配下から `\frametitle{...}` の引数テキストを探索して抽出する。
fn find_frametitle_in_node(node: &SyntaxNode) -> Option<String> {
    let mut tokens = node
        .descendants_with_tokens()
        .filter_map(|el| el.into_token());
    while let Some(tok) = tokens.next() {
        if tok.kind() == SyntaxKind::Command && tok.text() == "\\frametitle" {
            // `\frametitle` の後に続く開き波括弧 `{` を探索する。
            for next_tok in tokens.by_ref() {
                if next_tok.kind() == SyntaxKind::LBrace {
                    // 波括弧内のテキストを結合する。
                    let mut title = String::new();
                    let mut depth = 1;
                    for body_tok in tokens.by_ref() {
                        if body_tok.kind() == SyntaxKind::LBrace {
                            depth += 1;
                            title.push_str(body_tok.text());
                        } else if body_tok.kind() == SyntaxKind::RBrace {
                            depth -= 1;
                            if depth == 0 {
                                return Some(title.trim().to_string());
                            }
                            title.push_str(body_tok.text());
                        } else {
                            title.push_str(body_tok.text());
                        }
                    }
                    return Some(title.trim().to_string());
                } else if !next_tok.kind().is_trivia() {
                    break;
                }
            }
        }
    }
    None
}
