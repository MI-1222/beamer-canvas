//! スライドキャッシュキー生成およびコンテンツハッシュ管理モジュールのテスト。

use beamer_core::cache::{AssetDependency, ContentHasher};

#[test]
fn test_compute_hash() {
    let hash_a = ContentHasher::compute_hash("hello");
    let hash_b = ContentHasher::compute_hash("hello");
    let hash_c = ContentHasher::compute_hash("world");

    assert_eq!(hash_a, hash_b);
    assert_ne!(hash_a, hash_c);
}

#[test]
fn test_composite_hash_determinism_and_sensitivity() {
    let preamble = "\\documentclass{beamer}";
    let slide = "\\begin{frame}{Title}\\end{frame}";
    let assets = vec![
        AssetDependency::new("figs/chart.png", 1024, 1_700_000_000_000_000_000),
        AssetDependency::new("figs/logo.pdf", 2048, 1_700_000_001_000_000_000),
    ];

    let hash_base = ContentHasher::compute_composite_hash(preamble, slide, &assets, 150, 0);

    // 順序を反転したアセット一覧でも同一ハッシュの導出を確認。
    let mut reversed_assets = assets.clone();
    reversed_assets.reverse();
    let hash_reversed =
        ContentHasher::compute_composite_hash(preamble, slide, &reversed_assets, 150, 0);
    assert_eq!(hash_base, hash_reversed);

    // プリアンブル変更に伴うハッシュ値の変動を確認。
    let hash_preamble_changed = ContentHasher::compute_composite_hash(
        "\\documentclass[10pt]{beamer}",
        slide,
        &assets,
        150,
        0,
    );
    assert_ne!(hash_base, hash_preamble_changed);

    // スライド本文の変更に伴うハッシュ値の変動を確認。
    let hash_slide_changed = ContentHasher::compute_composite_hash(
        preamble,
        "\\begin{frame}{New Title}\\end{frame}",
        &assets,
        150,
        0,
    );
    assert_ne!(hash_base, hash_slide_changed);

    // アセットのタイムスタンプ更新に伴うハッシュ値の変動を確認。
    let mut modified_assets = assets.clone();
    modified_assets[0].modified_timestamp_nanos += 1;
    let hash_asset_modified =
        ContentHasher::compute_composite_hash(preamble, slide, &modified_assets, 150, 0);
    assert_ne!(hash_base, hash_asset_modified);

    // `DPI` 変更に伴うハッシュ値の変動を確認。
    let hash_dpi_changed = ContentHasher::compute_composite_hash(preamble, slide, &assets, 300, 0);
    assert_ne!(hash_base, hash_dpi_changed);

    // オーバーレイステップ変更に伴うハッシュ値の変動を確認。
    let hash_step_changed = ContentHasher::compute_composite_hash(preamble, slide, &assets, 150, 1);
    assert_ne!(hash_base, hash_step_changed);
}
