//! キャッシュキー生成およびコンテンツハッシュ管理モジュール。
//!
//! スライド単位の変更検知、プリアンブル変更、外部画像アセットの更新、
//! およびレンダリングパラメータ(`DPI`、オーバーレイステップ)を合成した
//! 決定論的なキャッシュキーの生成を提供する。

use serde::{Deserialize, Serialize};

/// スライドが依存する外部アセット(画像やインクルードファイル)のメタデータ。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AssetDependency {
    /// 外部ファイルの相対パス。
    pub path: String,
    /// ファイルサイズ(バイト)。
    pub file_size: u64,
    /// ファイルの最終更新日時(`UNIX`エポックからの経過ナノ秒)。
    pub modified_timestamp_nanos: u128,
}

impl AssetDependency {
    /// 新しい外部アセット依存情報を生成する。
    pub fn new(path: impl Into<String>, file_size: u64, modified_timestamp_nanos: u128) -> Self {
        Self {
            path: path.into(),
            file_size,
            modified_timestamp_nanos,
        }
    }
}

/// スライド単位の描画結果を一意に識別するための複合キャッシュキー構造体。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlideCacheKey {
    /// プリアンブルテキストのハッシュ値。
    pub preamble_hash: String,
    /// スライド本体テキスト(`CST`構文木テキスト)のハッシュ値。
    pub slide_source_hash: String,
    /// スライドが参照している外部アセットの一覧。
    pub assets: Vec<AssetDependency>,
    /// レンダリング解像度の `DPI` 値。
    pub dpi: u16,
    /// スライド内のオーバーレイステップ番号(0開始)。
    pub overlay_step: usize,
}

impl SlideCacheKey {
    /// 新しいスライドキャッシュキーを生成する。
    pub fn new(
        preamble_hash: impl Into<String>,
        slide_source_hash: impl Into<String>,
        assets: Vec<AssetDependency>,
        dpi: u16,
        overlay_step: usize,
    ) -> Self {
        Self {
            preamble_hash: preamble_hash.into(),
            slide_source_hash: slide_source_hash.into(),
            assets,
            dpi,
            overlay_step,
        }
    }

    /// 全構成要素を合成した決定論的な `BLAKE3` ハッシュ文字列(64文字の16進数)を算出する。
    pub fn to_composite_hash(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"beamer_slide_cache_v1\n");

        hasher.update(b"preamble:");
        hasher.update(self.preamble_hash.as_bytes());
        hasher.update(b"\n");

        hasher.update(b"slide:");
        hasher.update(self.slide_source_hash.as_bytes());
        hasher.update(b"\n");

        hasher.update(b"dpi:");
        hasher.update(&self.dpi.to_le_bytes());
        hasher.update(b"\n");

        hasher.update(b"overlay_step:");
        hasher.update(&self.overlay_step.to_le_bytes());
        hasher.update(b"\n");

        // アセットの順序依存を排除するため、ソートして更新する。
        let mut sorted_assets = self.assets.clone();
        sorted_assets.sort();

        hasher.update(b"assets_count:");
        hasher.update(&sorted_assets.len().to_le_bytes());
        hasher.update(b"\n");

        for asset in &sorted_assets {
            hasher.update(b"asset_path:");
            hasher.update(asset.path.as_bytes());
            hasher.update(b"\nasset_size:");
            hasher.update(&asset.file_size.to_le_bytes());
            hasher.update(b"\nasset_mtime:");
            hasher.update(&asset.modified_timestamp_nanos.to_le_bytes());
            hasher.update(b"\n");
        }

        hasher.finalize().to_hex().to_string()
    }
}

/// スライドコードや設定からコンテンツハッシュ(`BLAKE3`)を算出して一意のキーを生成する構造体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentHasher;

impl ContentHasher {
    /// 与えられたテキストコンテンツから `BLAKE3` ハッシュ文字列(16進数表記)を生成する。
    pub fn compute_hash(content: &str) -> String {
        let hash = blake3::hash(content.as_bytes());
        hash.to_hex().to_string()
    }

    /// スライドのソースコードとプリアンブルを連結した簡易ハッシュキーを生成する。
    pub fn compute_slide_key(preamble: &str, slide_source: &str) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(preamble.as_bytes());
        hasher.update(b"\n---slide_separator---\n");
        hasher.update(slide_source.as_bytes());
        hasher.finalize().to_hex().to_string()
    }

    /// 各構成要素から複合スライドキャッシュキーを生成する。
    pub fn compute_composite_key(
        preamble: &str,
        slide_source: &str,
        assets: &[AssetDependency],
        dpi: u16,
        overlay_step: usize,
    ) -> SlideCacheKey {
        let preamble_hash = Self::compute_hash(preamble);
        let slide_source_hash = Self::compute_hash(slide_source);
        SlideCacheKey::new(
            preamble_hash,
            slide_source_hash,
            assets.to_vec(),
            dpi,
            overlay_step,
        )
    }

    /// 各構成要素から最終的な `BLAKE3` 合成ハッシュ文字列を直接算出する。
    pub fn compute_composite_hash(
        preamble: &str,
        slide_source: &str,
        assets: &[AssetDependency],
        dpi: u16,
        overlay_step: usize,
    ) -> String {
        Self::compute_composite_key(preamble, slide_source, assets, dpi, overlay_step)
            .to_composite_hash()
    }
}
