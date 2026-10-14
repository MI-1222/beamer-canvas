//! 多層スライドキャッシュ(`L1`メモリおよび`L2`ディスク)の取得性能計測ベンチマーク。
//!
//! 150DPIおよび300DPIスライドビットマップに対する
//! `L1`メモリキャッシュヒット時間、`L2`ディスク`WebP`デコード復元時間、
//! および`BLAKE3`複合キャッシュキー算出のレイテンシを計測する。

use beamer_core::cache::{AssetDependency, ContentHasher};
use beamer_renderer::cache::{SlideCache, SlideCacheConfig};
use beamer_renderer::pdf::PageBitmap;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use tempfile::tempdir;

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

/// `L1`メモリキャッシュからの即時取得性能ベンチマーク。
fn bench_l1_memory_cache_hit(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_l1_memory_hit");

    let dir = tempdir().expect("一時ディレクトリの作成に失敗。");
    let config = SlideCacheConfig {
        cache_dir: dir.path().to_path_buf(),
        max_memory_bytes: 256 * 1024 * 1024,
    };
    let cache = SlideCache::new(config).expect("キャッシュの初期化に失敗。");

    // 150DPI(1440x810)および300DPI(2880x1620)のケース。
    let test_cases = [("150_dpi", 1440, 810), ("300_dpi", 2880, 1620)];

    for (name, width, height) in test_cases {
        let bitmap = create_test_bitmap(width, height, 42);
        let key = format!("bench_key_{name}");
        cache
            .insert_bitmap(&key, 0, bitmap)
            .expect("ビットマップの登録に失敗。");

        group.bench_with_input(BenchmarkId::from_parameter(name), &key, |b, k| {
            b.iter(|| {
                let loaded = cache.get_bitmap(black_box(k), black_box(0));
                black_box(loaded);
            });
        });
    }

    group.finish();
}

/// `L2`ディスクキャッシュ(`WebP`デコード)からの復元性能ベンチマーク。
fn bench_l2_disk_cache_restore(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_l2_disk_restore");

    let dir = tempdir().expect("一時ディレクトリの作成に失敗。");
    let config = SlideCacheConfig {
        cache_dir: dir.path().to_path_buf(),
        max_memory_bytes: 256 * 1024 * 1024,
    };
    let cache = SlideCache::new(config).expect("キャッシュの初期化に失敗。");

    let bitmap = create_test_bitmap(1440, 810, 77);
    let key = "bench_l2_restore_150dpi";
    cache
        .insert_bitmap(key, 0, bitmap)
        .expect("ビットマップの登録に失敗。");

    group.bench_function("150_dpi_webp_decode", |b| {
        b.iter(|| {
            // `L1`メモリキャッシュをクリアして必ず`L2`ディスクから読み出す。
            cache.clear_memory_cache();
            let loaded = cache.get_bitmap(black_box(key), black_box(0));
            black_box(loaded);
        });
    });

    group.finish();
}

/// 複合コンテンツハッシュキー算出のベンチマーク。
fn bench_composite_hash_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("composite_hash_computation");

    let preamble =
        "\\documentclass[aspectratio=169]{beamer}\n\\usetheme{Madrid}\n\\usecolortheme{beaver}";
    let slide = "\\begin{frame}{Performance Benchmark}\n  \\includegraphics[width=0.8\\textwidth]{chart.png}\n  \\begin{itemize}\n    \\item Item 1\n    \\item Item 2\n  \\end{itemize}\n\\end{frame}";
    let assets = vec![
        AssetDependency::new("figures/chart.png", 1024 * 200, 1_700_000_000_000),
        AssetDependency::new("macros/custom.sty", 1024 * 10, 1_700_000_005_000),
    ];
    let dpi = 150;
    let step = 0;

    group.bench_function("blake3_composite_hash", |b| {
        b.iter(|| {
            let hash = ContentHasher::compute_composite_hash(
                black_box(preamble),
                black_box(slide),
                black_box(&assets),
                black_box(dpi),
                black_box(step),
            );
            black_box(hash);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_l1_memory_cache_hit,
    bench_l2_disk_cache_restore,
    bench_composite_hash_computation
);
criterion_main!(benches);
