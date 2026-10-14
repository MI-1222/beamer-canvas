//! スライドキャッシュのヒット判定、コンパイルバイパス、およびレイテンシ要件の検証テスト。
//!
//! キャッシュ未存在時のビルド・画像化からキャッシュ投入、
//! 2回目以降のアクセスにおける `TeX` コンパイルの完全スキップ、
//! および `L1`メモリキャッシュと `L2`ディスクキャッシュからの10ms未満でのプレビュー取得を検証する。

use beamer_core::cache::{AssetDependency, ContentHasher};
use beamer_renderer::cache::{SlideCache, SlideCacheConfig};
use beamer_renderer::pdf::{PageBitmap, PdfRasterizer, RenderOptions};
use beamer_renderer::tex::{BuildManager, BuildRequest, TexCommandKind, TexEnvironment};
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use tempfile::TempDir;

/// テスト用のダミー `RGBA` ビットマップを生成する。
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

/// 利用可能な `TeX` エンジンを検出する。見つからない場合は `None` を返却。
async fn detect_available_engine() -> Option<TexCommandKind> {
    let env = TexEnvironment::detect().await;
    if env.is_available(TexCommandKind::Lualatex) {
        Some(TexCommandKind::Lualatex)
    } else if env.is_available(TexCommandKind::Latexmk) {
        Some(TexCommandKind::Latexmk)
    } else if env.is_available(TexCommandKind::Pdflatex) {
        Some(TexCommandKind::Pdflatex)
    } else if env.is_available(TexCommandKind::Xelatex) {
        Some(TexCommandKind::Xelatex)
    } else {
        None
    }
}

#[tokio::test]
async fn test_cache_hit_bypasses_compilation_with_sub_10ms_latency() {
    let Some(engine) = detect_available_engine().await else {
        println!(
            "実行環境に `TeX` コマンドが見つからないため、キャッシュバイパステストをスキップ。"
        );
        return;
    };

    let Ok(rasterizer) = PdfRasterizer::new() else {
        println!("`PDFium` ライブラリが見つからないため、キャッシュバイパステストをスキップ。");
        return;
    };

    let temp_dir = TempDir::new().expect("一時ディレクトリの作成に失敗。");
    let project_root = temp_dir.path().to_path_buf();
    let cache_dir = project_root.join(".beamer_cache");

    let cache_config = SlideCacheConfig {
        cache_dir: cache_dir.clone(),
        max_memory_bytes: 64 * 1024 * 1024,
    };
    let cache = SlideCache::new(cache_config).expect("キャッシュの初期化に失敗。");

    let preamble = "\\documentclass{beamer}\n\\usetheme{default}\n\\begin{document}";
    let slide_content =
        "\\begin{frame}{Performance Test}\n  Testing low latency cache.\n\\end{frame}";
    let full_source = format!("{preamble}\n{slide_content}\n\\end{{document}}");

    let main_file = PathBuf::from("main.tex");
    fs::write(project_root.join(&main_file), &full_source)
        .expect("`TeX` ファイルの書き込みに失敗。");

    let dpi = 150;
    let step = 0;
    let empty_assets: Vec<AssetDependency> = Vec::new();
    let composite_hash =
        ContentHasher::compute_composite_hash(preamble, slide_content, &empty_assets, dpi, step);

    // 1. 初回要求(`Cold Run`): キャッシュミスの確認。
    assert!(
        cache.get_bitmap(&composite_hash, step).is_none(),
        "初回のキャッシュミス判定に失敗。"
    );

    // キャッシュミス時はコンパイルを実行してレンダリング。
    let manager = BuildManager::new();
    let mut request = BuildRequest::full(&project_root, &main_file);
    request.engine = Some(engine);

    let cold_start = Instant::now();
    let build_result = manager
        .request_build(request)
        .await
        .expect("初回ビルドの実行に失敗。");
    assert!(build_result.success, "コンパイル処理に失敗。");

    let pdf_path = build_result.pdf_path.expect("`PDF` 出力の確認に失敗。");
    let render_opts = RenderOptions::new().with_dpi(dpi);
    let rendered_bitmap = rasterizer
        .render_page_to_rgba_with_options(&pdf_path, 0, &render_opts)
        .expect("初回のラスタライズに失敗。");
    let cold_duration = cold_start.elapsed();

    // キャッシュへの登録。
    cache
        .insert_bitmap(&composite_hash, step, rendered_bitmap.clone())
        .expect("キャッシュへのビットマップ登録に失敗。");

    println!("初回コンパイル＋画像化所要時間: {:?}", cold_duration);

    // 2. 2回目要求(`Warm Run`/`L1`メモリキャッシュヒット): コンパイルを完全スキップ。
    // `BuildManager` を呼び出さずに `SlideCache` から取得。
    let warm_start = Instant::now();
    let cached_bitmap = cache.get_bitmap(&composite_hash, step);
    let warm_duration = warm_start.elapsed();

    println!("キャッシュヒット(`L1`メモリ)所要時間: {:?}", warm_duration);

    assert!(
        cached_bitmap.is_some(),
        "キャッシュからのビットマップ取得に失敗。"
    );
    let cached_bitmap = cached_bitmap.unwrap();
    assert_eq!(cached_bitmap.width, rendered_bitmap.width);
    assert_eq!(cached_bitmap.height, rendered_bitmap.height);
    assert_eq!(cached_bitmap.rgba_data, rendered_bitmap.rgba_data);

    // 判定基準: 取得時間が10ms未満であること。
    assert!(
        warm_duration < std::time::Duration::from_millis(10),
        "`L1`メモリキャッシュからの取得レイテンシが10msを超過したため失敗。(実測: {:?})",
        warm_duration
    );
}

#[test]
fn test_l1_memory_cache_latency_sub_10ms_repeated() {
    let temp_dir = TempDir::new().expect("一時ディレクトリの作成に失敗。");
    let cache_config = SlideCacheConfig {
        cache_dir: temp_dir.path().to_path_buf(),
        max_memory_bytes: 64 * 1024 * 1024,
    };
    let cache = SlideCache::new(cache_config).expect("キャッシュの初期化に失敗。");

    // 150`DPI`スライド実寸大相当(1440x810ピクセル)。
    let width = 1440;
    let height = 810;
    let bitmap = create_test_bitmap(width, height, 42);
    let hash_key = "benchmark_composite_key_150dpi_f1";
    let step = 0;

    cache
        .insert_bitmap(hash_key, step, bitmap.clone())
        .expect("ビットマップの登録に失敗。");

    // ウォームアップ実行。
    for _ in 0..5 {
        let _ = cache.get_bitmap(hash_key, step);
    }

    // 50回の反復取得によるレイテンシ計測。
    let iterations = 50;
    let mut total_duration = std::time::Duration::ZERO;
    let mut max_duration = std::time::Duration::ZERO;

    for _ in 0..iterations {
        let start = Instant::now();
        let loaded = cache.get_bitmap(hash_key, step);
        let elapsed = start.elapsed();

        assert!(loaded.is_some(), "反復取得時のキャッシュ読み出しに失敗。");
        total_duration += elapsed;
        if elapsed > max_duration {
            max_duration = elapsed;
        }
    }

    let avg_duration = total_duration / iterations;
    println!(
        "`L1`メモリキャッシュ反復計測({}回平均): {:?}, 最大: {:?}",
        iterations, avg_duration, max_duration
    );

    // 平均所要時間が10ms未満であることを検証。
    assert!(
        avg_duration < std::time::Duration::from_millis(10),
        "平均取得レイテンシが10msを超過したため失敗。(実測: {:?})",
        avg_duration
    );
}

#[test]
fn test_l2_disk_cache_restore_and_latency() {
    let temp_dir = TempDir::new().expect("一時ディレクトリの作成に失敗。");
    let cache_config = SlideCacheConfig {
        cache_dir: temp_dir.path().to_path_buf(),
        max_memory_bytes: 64 * 1024 * 1024,
    };
    let cache = SlideCache::new(cache_config).expect("キャッシュの初期化に失敗。");

    let width = 960;
    let height = 540;
    let original_bitmap = create_test_bitmap(width, height, 100);
    let hash_key = "disk_cache_performance_test_key";
    let step = 0;

    // ディスクとメモリへの保存。
    cache
        .insert_bitmap(hash_key, step, original_bitmap.clone())
        .expect("ビットマップの保存に失敗。");

    // `L1`メモリキャッシュを意図的に完全クリア。
    cache.clear_memory_cache();
    assert_eq!(cache.memory_entry_count(), 0);

    // `L2`ディスクキャッシュ(`WebP`ロスレス)からの復元時間を計測。
    let start = Instant::now();
    let restored = cache.get_bitmap(hash_key, step);
    let elapsed = start.elapsed();

    println!("`L2`ディスクキャッシュ復元所要時間: {:?}", elapsed);

    assert!(
        restored.is_some(),
        "`L2`ディスクキャッシュからの復元に失敗。"
    );
    let restored = restored.unwrap();
    assert_eq!(restored.width, original_bitmap.width);
    assert_eq!(restored.height, original_bitmap.height);
    assert_eq!(restored.rgba_data, original_bitmap.rgba_data);

    // 復元後の`L1`メモリキャッシュ再配置を確認。
    cache.sync();
    assert_eq!(cache.memory_entry_count(), 1);
}

#[test]
fn test_cache_invalidation_sensitivity() {
    let temp_dir = TempDir::new().expect("一時ディレクトリの作成に失敗。");
    let cache_config = SlideCacheConfig {
        cache_dir: temp_dir.path().to_path_buf(),
        max_memory_bytes: 32 * 1024 * 1024,
    };
    let cache = SlideCache::new(cache_config).expect("キャッシュの初期化に失敗。");

    let base_preamble = "\\documentclass{beamer}\\usetheme{default}";
    let base_slide = "\\begin{frame}{Sensitivity Test}\\end{frame}";
    let asset = AssetDependency::new("chart.png", 4096, 1_700_000_000_000);
    let base_assets = vec![asset.clone()];
    let base_dpi = 150;
    let base_step = 0;

    let base_hash = ContentHasher::compute_composite_hash(
        base_preamble,
        base_slide,
        &base_assets,
        base_dpi,
        base_step,
    );

    let base_bitmap = create_test_bitmap(64, 64, 7);
    cache
        .insert_bitmap(&base_hash, base_step, base_bitmap.clone())
        .expect("ベースビットマップの保存に失敗。");

    // 1. スライド本体の変更によるキャッシュミス。
    let modified_slide = "\\begin{frame}{Sensitivity Test Modified}\\end{frame}";
    let hash_slide_mod = ContentHasher::compute_composite_hash(
        base_preamble,
        modified_slide,
        &base_assets,
        base_dpi,
        base_step,
    );
    assert_ne!(base_hash, hash_slide_mod);
    assert!(
        cache.get_bitmap(&hash_slide_mod, base_step).is_none(),
        "スライド本文変更時のキャッシュ無効化に失敗。"
    );

    // 2. プリアンブルの変更によるキャッシュミス。
    let modified_preamble = "\\documentclass{beamer}\\usetheme{Madrid}";
    let hash_preamble_mod = ContentHasher::compute_composite_hash(
        modified_preamble,
        base_slide,
        &base_assets,
        base_dpi,
        base_step,
    );
    assert_ne!(base_hash, hash_preamble_mod);
    assert!(
        cache.get_bitmap(&hash_preamble_mod, base_step).is_none(),
        "プリアンブル変更時のキャッシュ無効化に失敗。"
    );

    // 3. 外部アセット更新(更新日時の変動)によるキャッシュミス。
    let mut updated_asset = asset.clone();
    updated_asset.modified_timestamp_nanos += 1_000_000;
    let hash_asset_mod = ContentHasher::compute_composite_hash(
        base_preamble,
        base_slide,
        &[updated_asset],
        base_dpi,
        base_step,
    );
    assert_ne!(base_hash, hash_asset_mod);
    assert!(
        cache.get_bitmap(&hash_asset_mod, base_step).is_none(),
        "外部画像更新時のキャッシュ無効化に失敗。"
    );

    // 4. `DPI`変更によるキャッシュミス。
    let hash_dpi_mod = ContentHasher::compute_composite_hash(
        base_preamble,
        base_slide,
        &base_assets,
        300,
        base_step,
    );
    assert_ne!(base_hash, hash_dpi_mod);
    assert!(
        cache.get_bitmap(&hash_dpi_mod, base_step).is_none(),
        "`DPI`変更時のキャッシュ無効化に失敗。"
    );

    // 5. アニメーションステップの変更による独立性。
    let step_1 = 1;
    let hash_step_1 = ContentHasher::compute_composite_hash(
        base_preamble,
        base_slide,
        &base_assets,
        base_dpi,
        step_1,
    );
    assert_ne!(base_hash, hash_step_1);
    assert!(
        cache.get_bitmap(&hash_step_1, step_1).is_none(),
        "未登録ステップのキャッシュミス判定に失敗。"
    );

    // 元のキャッシュエントリが保持されていることを確認。
    let retrieved_base = cache.get_bitmap(&base_hash, base_step);
    assert!(
        retrieved_base.is_some(),
        "元のキャッシュエントリの取得に失敗。"
    );
    assert_eq!(
        retrieved_base.unwrap().rgba_data,
        base_bitmap.rgba_data,
        "保持されたビットマップデータの整合性検証に失敗。"
    );
}
