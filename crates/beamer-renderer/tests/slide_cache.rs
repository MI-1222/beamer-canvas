//! スライドキャッシュ(L1メモリおよびL2ディスク)のテストスイート。

use beamer_core::cache::{AssetDependency, ContentHasher};
use beamer_renderer::cache::{SlideCache, SlideCacheConfig};
use beamer_renderer::pdf::PageBitmap;
use std::sync::Arc;
use std::thread;
use tempfile::tempdir;

/// テスト用のダミー `RGBA` ビットマップの生成。
fn create_test_bitmap(width: u32, height: u32, seed_byte: u8) -> PageBitmap {
    let pixel_count = (width * height) as usize;
    let mut rgba_data = Vec::with_capacity(pixel_count * 4);
    for i in 0..pixel_count {
        let val = (seed_byte as usize + i) % 256;
        rgba_data.push(val as u8);
        rgba_data.push((val + 50) as u8);
        rgba_data.push((val + 100) as u8);
        rgba_data.push(255);
    }
    PageBitmap {
        page_index: 0,
        width,
        height,
        rgba_data,
    }
}

#[test]
fn test_slide_cache_memory_and_disk_roundtrip() {
    let dir = tempdir().expect("一時ディレクトリの作成に失敗。");
    let config = SlideCacheConfig {
        cache_dir: dir.path().to_path_buf(),
        max_memory_bytes: 10 * 1024 * 1024,
    };
    let cache = SlideCache::new(config).expect("キャッシュの初期化に失敗。");

    let hash_key = "a1b2c3d4e5f6";
    let step = 0;
    let original_bitmap = create_test_bitmap(16, 16, 42);

    // 1. キャッシュ未登録時は `None` を返却。
    assert!(cache.get_bitmap(hash_key, step).is_none());

    // 2. ビットマップの登録。
    cache
        .insert_bitmap(hash_key, step, original_bitmap.clone())
        .expect("ビットマップの保存に失敗。");

    // ディスク上への `WebP` ファイル出力の確認。
    let disk_file = dir.path().join(format!("{hash_key}_{step}.webp"));
    assert!(disk_file.exists());

    // 3. L1メモリからの即時取得を確認。
    let loaded = cache
        .get_bitmap(hash_key, step)
        .expect("L1キャッシュからの取得に失敗。");
    assert_eq!(loaded.width, original_bitmap.width);
    assert_eq!(loaded.height, original_bitmap.height);
    assert_eq!(loaded.rgba_data, original_bitmap.rgba_data);

    // 4. L1メモリキャッシュ消去後、L2ディスクから `WebP` 経由での復元を確認。
    cache.clear_memory_cache();
    assert_eq!(cache.memory_entry_count(), 0);

    let restored = cache
        .get_bitmap(hash_key, step)
        .expect("L2ディスクからの復元に失敗。");
    assert_eq!(restored.width, original_bitmap.width);
    assert_eq!(restored.height, original_bitmap.height);
    assert_eq!(restored.rgba_data, original_bitmap.rgba_data);

    // 復元処理に伴うL1メモリへの再配置を確認。
    cache.sync();
    assert_eq!(cache.memory_entry_count(), 1);
}

#[test]
fn test_slide_cache_pdf_roundtrip() {
    let dir = tempdir().expect("一時ディレクトリの作成に失敗。");
    let config = SlideCacheConfig {
        cache_dir: dir.path().to_path_buf(),
        max_memory_bytes: 10 * 1024 * 1024,
    };
    let cache = SlideCache::new(config).expect("キャッシュの初期化に失敗。");

    let hash_key = "pdf_hash_12345";
    let dummy_pdf = b"%PDF-1.5 dummy pdf content for testing";

    assert!(cache.get_pdf(hash_key).is_none());

    cache
        .insert_pdf(hash_key, dummy_pdf)
        .expect("`PDF`の保存に失敗。");

    let loaded = cache.get_pdf(hash_key).expect("`PDF`の取得に失敗。");
    assert_eq!(loaded, dummy_pdf);
}

#[test]
fn test_slide_cache_eviction_by_memory_weight() {
    let dir = tempdir().expect("一時ディレクトリの作成に失敗。");
    // 最大メモリ容量を小さめに設定(2枚分入るが3枚目は超過する設定)。
    // 4x4 RGBA = 64 bytes
    let bitmap1 = create_test_bitmap(4, 4, 1);
    let bitmap2 = create_test_bitmap(4, 4, 2);
    let bitmap3 = create_test_bitmap(4, 4, 3);
    let single_size = bitmap1.rgba_data.len() as u64;

    let config = SlideCacheConfig {
        cache_dir: dir.path().to_path_buf(),
        max_memory_bytes: single_size * 2 + 10,
    };
    let cache = SlideCache::new(config).expect("キャッシュの初期化に失敗。");

    cache
        .insert_bitmap("key1", 0, bitmap1)
        .expect("`key1` の保存に失敗。");
    cache
        .insert_bitmap("key2", 0, bitmap2)
        .expect("`key2` の保存に失敗。");
    cache
        .insert_bitmap("key3", 0, bitmap3)
        .expect("`key3` の保存に失敗。");

    // 保留中メンテナンスタスクの同期実行。
    cache.sync();

    // 3枚登録後も容量制限によりメモリ使用量が `max_memory_bytes` 以内に維持されることを確認。
    assert!(cache.memory_usage_bytes() <= cache.config().max_memory_bytes);
}

#[test]
fn test_slide_cache_composite_key_integration() {
    let dir = tempdir().expect("一時ディレクトリの作成に失敗。");
    let config = SlideCacheConfig {
        cache_dir: dir.path().to_path_buf(),
        max_memory_bytes: 10 * 1024 * 1024,
    };
    let cache = SlideCache::new(config).expect("キャッシュの初期化に失敗。");

    let preamble = "\\documentclass{beamer}\\usetheme{Madrid}";
    let slide = "\\begin{frame}{Demo}\\includegraphics{logo.png}\\end{frame}";
    let assets = vec![AssetDependency::new("logo.png", 5000, 1_700_000_000_000)];
    let dpi = 150;
    let step = 0;

    let composite_hash = ContentHasher::compute_composite_hash(preamble, slide, &assets, dpi, step);

    let bitmap = create_test_bitmap(8, 8, 99);
    cache
        .insert_bitmap(&composite_hash, step, bitmap.clone())
        .expect("ビットマップの保存に失敗。");

    let retrieved = cache
        .get_bitmap(&composite_hash, step)
        .expect("キャッシュの取得に失敗。");
    assert_eq!(retrieved.rgba_data, bitmap.rgba_data);

    // 画像更新日時の変更に伴うキャッシュミスを確認。
    let modified_assets = vec![AssetDependency::new("logo.png", 5000, 1_700_000_001_000)];
    let new_hash =
        ContentHasher::compute_composite_hash(preamble, slide, &modified_assets, dpi, step);
    assert_ne!(composite_hash, new_hash);
    assert!(cache.get_bitmap(&new_hash, step).is_none());
}

#[test]
fn test_slide_cache_clear_disk() {
    let dir = tempdir().expect("一時ディレクトリの作成に失敗。");
    let config = SlideCacheConfig {
        cache_dir: dir.path().to_path_buf(),
        max_memory_bytes: 10 * 1024 * 1024,
    };
    let cache = SlideCache::new(config).expect("キャッシュの初期化に失敗。");

    let bitmap = create_test_bitmap(4, 4, 10);
    cache
        .insert_bitmap("key_a", 0, bitmap)
        .expect("`key_a` の保存に失敗。");
    cache
        .insert_pdf("key_b", b"pdf")
        .expect("`key_b` の保存に失敗。");

    assert!(dir.path().join("key_a_0.webp").exists());
    assert!(dir.path().join("key_b.pdf").exists());

    cache
        .clear_disk_cache()
        .expect("ディスクキャッシュの消去に失敗。");

    assert!(!dir.path().join("key_a_0.webp").exists());
    assert!(!dir.path().join("key_b.pdf").exists());
}

#[test]
fn test_concurrent_access_and_disk_atomic_safety() {
    let dir = tempdir().expect("一時ディレクトリの作成に失敗。");
    let config = SlideCacheConfig {
        cache_dir: dir.path().to_path_buf(),
        max_memory_bytes: 50 * 1024 * 1024,
    };
    let cache = Arc::new(SlideCache::new(config).expect("キャッシュの初期化に失敗。"));

    let preamble = "\\documentclass{beamer}";
    let slide = "\\begin{frame}{Concurrent Test}\\end{frame}";
    let assets = vec![AssetDependency::new("image.png", 100, 1000)];
    let hash = ContentHasher::compute_composite_hash(preamble, slide, &assets, 150, 0);

    // 複数スレッドからの同一キーに対する並行読み書き。
    let mut handles = Vec::new();
    for thread_idx in 0..8 {
        let cache_clone = Arc::clone(&cache);
        let hash_clone = hash.clone();
        handles.push(thread::spawn(move || {
            let bitmap = create_test_bitmap(32, 32, thread_idx as u8);
            cache_clone
                .insert_bitmap(&hash_clone, 0, bitmap)
                .expect("並行書き込みに失敗。");

            let fetched = cache_clone.get_bitmap(&hash_clone, 0);
            assert!(fetched.is_some());
            let fetched = fetched.unwrap();
            assert_eq!(fetched.width, 32);
            assert_eq!(fetched.height, 32);
        }));
    }

    for handle in handles {
        handle.join().expect("スレッドの実行が原因で失敗しました。");
    }

    // ディスク上のファイル破損がなく、正常にデコードできることを確認。
    cache.clear_memory_cache();
    let restored = cache
        .get_bitmap(&hash, 0)
        .expect("並行書き込み後のディスク復元に失敗。");
    assert_eq!(restored.width, 32);
    assert_eq!(restored.height, 32);
}

#[test]
fn test_composite_hash_invalidation_flow() {
    let dir = tempdir().expect("一時ディレクトリの作成に失敗。");
    let config = SlideCacheConfig {
        cache_dir: dir.path().to_path_buf(),
        max_memory_bytes: 10 * 1024 * 1024,
    };
    let cache = SlideCache::new(config).expect("キャッシュの初期化に失敗。");

    let preamble = "\\documentclass{beamer}\\usetheme{default}";
    let slide_source = "\\begin{frame}{Slide 1}\\end{frame}";
    let asset = AssetDependency::new("graph.pdf", 2048, 1_700_000_000);

    // 1. 初回ビルドラスタライズ結果のキャッシュ格納。
    let hash_v1 = ContentHasher::compute_composite_hash(
        preamble,
        slide_source,
        std::slice::from_ref(&asset),
        150,
        0,
    );
    let bitmap_v1 = create_test_bitmap(16, 16, 10);
    cache
        .insert_bitmap(&hash_v1, 0, bitmap_v1.clone())
        .expect("`hash_v1` の保存に失敗。");

    // キャッシュヒットの確認。
    let hit = cache
        .get_bitmap(&hash_v1, 0)
        .expect("`hash_v1` の取得に失敗。");
    assert_eq!(hit.rgba_data, bitmap_v1.rgba_data);

    // 2. 外部画像ファイルのタイムスタンプ更新(ファイル差し替えの模擬)。
    let mut asset_updated = asset.clone();
    asset_updated.modified_timestamp_nanos += 500;

    let hash_v2 = ContentHasher::compute_composite_hash(
        preamble,
        slide_source,
        std::slice::from_ref(&asset_updated),
        150,
        0,
    );
    assert_ne!(hash_v1, hash_v2);

    // 新キーに対するキャッシュミスを確認。
    assert!(cache.get_bitmap(&hash_v2, 0).is_none());

    // 3. 新キーでのビットマップ保存。
    let bitmap_v2 = create_test_bitmap(16, 16, 20);
    cache
        .insert_bitmap(&hash_v2, 0, bitmap_v2.clone())
        .expect("`hash_v2` の保存に失敗。");

    // 各キーに応じた画像が独立して取得可能であることを確認。
    assert_eq!(
        cache
            .get_bitmap(&hash_v1, 0)
            .expect("`hash_v1` の取得に失敗。")
            .rgba_data,
        bitmap_v1.rgba_data
    );
    assert_eq!(
        cache
            .get_bitmap(&hash_v2, 0)
            .expect("`hash_v2` の取得に失敗。")
            .rgba_data,
        bitmap_v2.rgba_data
    );
}
