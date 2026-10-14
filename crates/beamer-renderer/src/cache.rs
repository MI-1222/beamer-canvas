//! スライド単位のコンテンツハッシュに対応した多層キャッシュモジュール。
//!
//! L1 メモリキャッシュ(`moka` による `LRU` メモリキャッシュ)と
//! L2 ディスクキャッシュ(`WebP` ロスレス画像および部分pdfのアトミック永続化)を提供する。

use crate::error::{RenderError, Result};
use crate::pdf::PageBitmap;
use moka::sync::Cache;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// スライドキャッシュの構成設定。
#[derive(Debug, Clone)]
pub struct SlideCacheConfig {
    /// ディスクキャッシュを配置するルートディレクトリパス。
    pub cache_dir: PathBuf,
    /// L1 メモリキャッシュの最大許容バイト数(デフォルト: 256MB)。
    pub max_memory_bytes: u64,
}

impl Default for SlideCacheConfig {
    fn default() -> Self {
        Self {
            cache_dir: PathBuf::from(".beamer_cache"),
            max_memory_bytes: 256 * 1024 * 1024,
        }
    }
}

/// L1 メモリキャッシュおよび L2 ディスクキャッシュを管理するキャッシュマネージャー。
pub struct SlideCache {
    config: SlideCacheConfig,
    memory_cache: Cache<String, PageBitmap>,
    temp_counter: AtomicU64,
}

impl SlideCache {
    /// 設定を指定して新しいスライドキャッシュを初期化する。
    pub fn new(config: SlideCacheConfig) -> Result<Self> {
        if !config.cache_dir.exists() {
            fs::create_dir_all(&config.cache_dir).map_err(|e| RenderError::Cache {
                message: format!(
                    "キャッシュディレクトリの作成に失敗しました: {}: {}",
                    config.cache_dir.display(),
                    e
                ),
            })?;
        }

        let memory_cache = Cache::builder()
            .weigher(|_key: &String, value: &PageBitmap| -> u32 {
                u32::try_from(value.rgba_data.len()).unwrap_or(u32::MAX)
            })
            .max_capacity(config.max_memory_bytes)
            .build();

        Ok(Self {
            config,
            memory_cache,
            temp_counter: AtomicU64::new(0),
        })
    }

    /// キャッシュディレクトリパスを取得する。
    #[inline]
    pub fn cache_dir(&self) -> &Path {
        &self.config.cache_dir
    }

    /// キャッシュ設定の参照を取得する。
    #[inline]
    pub fn config(&self) -> &SlideCacheConfig {
        &self.config
    }

    /// L1 メモリキャッシュで使用中の推定バイト数を取得する。
    #[inline]
    pub fn memory_usage_bytes(&self) -> u64 {
        self.memory_cache.weighted_size()
    }

    /// L1 メモリキャッシュのエントリ数を取得する。
    #[inline]
    pub fn memory_entry_count(&self) -> u64 {
        self.memory_cache.entry_count()
    }

    /// 指定したハッシュキーとオーバーレイステップに対応するビットマップを取得する。
    ///
    /// 1. L1 メモリキャッシュを探索し、存在すれば即座に返却する。
    /// 2. L2 ディスクキャッシュを探索し、存在すれば `WebP` をデコードして L1 に乗せて返却する。
    /// 3. いずれにも存在しない場合は `None` を返す。
    pub fn get_bitmap(&self, composite_hash: &str, overlay_step: usize) -> Option<PageBitmap> {
        let key = Self::format_bitmap_key(composite_hash, overlay_step);

        if let Some(bitmap) = self.memory_cache.get(&key) {
            return Some(bitmap);
        }

        // L2 ディスクキャッシュからの読み込み処理。
        let disk_path = self.bitmap_disk_path(composite_hash, overlay_step);
        if disk_path.is_file() {
            match self.load_bitmap_from_disk(&disk_path, overlay_step) {
                Ok(bitmap) => {
                    self.memory_cache.insert(key, bitmap.clone());
                    Some(bitmap)
                }
                Err(_) => None,
            }
        } else {
            None
        }
    }

    /// ビットマップを L1 メモリキャッシュおよび L2 ディスクキャッシュに格納する。
    pub fn insert_bitmap(
        &self,
        composite_hash: &str,
        overlay_step: usize,
        bitmap: PageBitmap,
    ) -> Result<()> {
        let key = Self::format_bitmap_key(composite_hash, overlay_step);
        let disk_path = self.bitmap_disk_path(composite_hash, overlay_step);

        self.save_bitmap_to_disk(&disk_path, &bitmap)?;
        self.memory_cache.insert(key, bitmap);

        Ok(())
    }

    /// 指定したハッシュキーに対応するビルド済み部分pdfを取得する。
    pub fn get_pdf(&self, composite_hash: &str) -> Option<Vec<u8>> {
        let disk_path = self.pdf_disk_path(composite_hash);
        if disk_path.is_file() {
            fs::read(&disk_path).ok()
        } else {
            None
        }
    }

    /// ビルド済み部分pdfを L2 ディスクキャッシュに保存する。
    pub fn insert_pdf(&self, composite_hash: &str, pdf_bytes: &[u8]) -> Result<()> {
        let disk_path = self.pdf_disk_path(composite_hash);
        self.atomic_write(&disk_path, pdf_bytes)
    }

    /// L1 メモリキャッシュをすべて破棄(パージ)する。
    pub fn clear_memory_cache(&self) {
        self.memory_cache.invalidate_all();
        self.memory_cache.run_pending_tasks();
    }

    /// キャッシュの保留中メンテナンスタスク(エビクションや登録処理)を同期実行する。
    pub fn sync(&self) {
        self.memory_cache.run_pending_tasks();
    }

    /// L2 ディスクキャッシュ内のファイルをすべて削除する。
    pub fn clear_disk_cache(&self) -> Result<()> {
        if self.config.cache_dir.exists() {
            for entry in fs::read_dir(&self.config.cache_dir).map_err(|e| RenderError::Cache {
                message: format!("キャッシュディレクトリの走査に失敗しました: {e}"),
            })? {
                let entry = entry.map_err(|e| RenderError::Cache {
                    message: format!("ディレクトリ走査エラー: {e}"),
                })?;
                let path = entry.path();
                if path.is_file() {
                    let _ = fs::remove_file(&path);
                }
            }
        }
        Ok(())
    }

    fn format_bitmap_key(composite_hash: &str, overlay_step: usize) -> String {
        format!("{composite_hash}_{overlay_step}")
    }

    fn bitmap_disk_path(&self, composite_hash: &str, overlay_step: usize) -> PathBuf {
        self.config
            .cache_dir
            .join(format!("{composite_hash}_{overlay_step}.webp"))
    }

    fn pdf_disk_path(&self, composite_hash: &str) -> PathBuf {
        self.config.cache_dir.join(format!("{composite_hash}.pdf"))
    }

    fn save_bitmap_to_disk(&self, target_path: &Path, bitmap: &PageBitmap) -> Result<()> {
        let encoder = webp::Encoder::from_rgba(&bitmap.rgba_data, bitmap.width, bitmap.height);
        let webp_data = encoder.encode_lossless();
        self.atomic_write(target_path, &webp_data)
    }

    fn load_bitmap_from_disk(&self, path: &Path, overlay_step: usize) -> Result<PageBitmap> {
        let data = fs::read(path).map_err(|e| RenderError::Cache {
            message: format!(
                "`WebP`ファイルの読み込みに失敗しました: {}: {}",
                path.display(),
                e
            ),
        })?;

        let decoder = webp::Decoder::new(&data);
        let image = decoder.decode().ok_or_else(|| RenderError::Cache {
            message: format!("`WebP`のデコードに失敗しました: {}", path.display()),
        })?;

        let width = image.width();
        let height = image.height();
        let rgba_data = image.to_image().to_rgba8().into_raw();

        Ok(PageBitmap {
            page_index: overlay_step,
            width,
            height,
            rgba_data,
        })
    }

    fn atomic_write(&self, target_path: &Path, content: &[u8]) -> Result<()> {
        if let Some(parent) = target_path.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent).map_err(|e| RenderError::Cache {
                message: format!("親ディレクトリの作成に失敗しました: {e}"),
            })?;
        }

        let count = self.temp_counter.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let temp_filename = format!(".tmp_{}_{}_{}", pid, count, fastrand_u64());
        let temp_path = self.config.cache_dir.join(temp_filename);

        fs::write(&temp_path, content).map_err(|e| RenderError::Cache {
            message: format!("一時キャッシュファイルの書き込みに失敗しました: {e}"),
        })?;

        fs::rename(&temp_path, target_path).map_err(|e| {
            let _ = fs::remove_file(&temp_path);
            RenderError::Cache {
                message: format!("キャッシュファイルのアトミック配置に失敗しました: {e}"),
            }
        })?;

        Ok(())
    }
}

/// 乱数シードを用いないシンプルな一時ファイルサフィックス生成。
fn fastrand_u64() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}
