//! `Beamer` パーサーおよび `CST` 編集 `API` の性能計測ベンチマーク。
//!
//! スライド規模(5枚、30枚、100枚)別のパース完了時間、
//! および大規模ドキュメントに対する編集操作のレイテンシを計測する。

use beamer_parser::parse;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

/// 指定したスライド枚数の `Beamer` ドキュメント文字列を生成する。
fn generate_beamer_document(slide_count: usize) -> String {
    let mut doc = String::with_capacity(slide_count * 500 + 200);
    doc.push_str(
        "\\documentclass[aspectratio=169]{beamer}\n\
         \\usetheme{Madrid}\n\
         \\title{Scalability Benchmark Document}\n\
         \\author{Benchmark Runner}\n\
         \\begin{document}\n\n",
    );

    for i in 1..=slide_count {
        doc.push_str(&format!(
            "\\section{{Section for Slide {i}}}\n\
             \\begin{{frame}}[t]{{Slide {i}: Analysis and Results}}\n\
             \\framesubtitle{{Subtitle detailing metrics and evaluations}}\n\
             Introduction text for slide {i} with inline math $x_{{{i}}} = \\alpha \\cdot \\beta$.\n\
             \\begin{{columns}}[T]\n\
             \\begin{{column}}{{0.48\\textwidth}}\n\
             \\begin{{block}}{{Block Title {i}}}\n\
             Key points regarding performance:\n\
             \\begin{{itemize}}\n\
             \\item First crucial observation on step 1.\n\
             \\item Second measurement parameter on step 2.\n\
             \\end{{itemize}}\n\
             \\end{{block}}\n\
             \\end{{column}}\n\
             \\begin{{column}}{{0.48\\textwidth}}\n\
             \\includegraphics[width=0.9\\linewidth]{{figures/chart_{i}.png}}\n\
             \\end{{column}}\n\
             \\end{{columns}}\n\
             \\end{{frame}}\n\n"
        ));
    }

    doc.push_str("\\end{document}\n");
    doc
}

/// 規模別(5枚、30枚、100枚)コールドパースのベンチマーク。
fn bench_cold_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_cold");

    for &slide_count in &[5, 30, 100] {
        let doc = generate_beamer_document(slide_count);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{slide_count}_slides")),
            &doc,
            |b, input| {
                b.iter(|| {
                    let parsed = parse(black_box(input));
                    black_box(parsed);
                });
            },
        );
    }

    group.finish();
}

/// 大規模ドキュメント(100枚)に対する `CST` 編集 `API` のレイテンシ計測。
fn bench_edit_operations_100_slides(c: &mut Criterion) {
    let doc = generate_beamer_document(100);
    let mut group = c.benchmark_group("edit_operations_100_slides");

    // スライドの並び替え(`swap_frames`)
    group.bench_function("swap_frames", |b| {
        b.iter(|| {
            let parsed = parse(black_box(&doc));
            let editor = parsed.editor();
            let result = editor.swap_frames(black_box(10), black_box(50));
            black_box(result).expect("スライドのスワップに失敗。");
        });
    });

    // スライドの移動(`move_frame`)
    group.bench_function("move_frame", |b| {
        b.iter(|| {
            let parsed = parse(black_box(&doc));
            let editor = parsed.editor();
            let result = editor.move_frame(black_box(5), black_box(80));
            black_box(result).expect("スライドの移動に失敗。");
        });
    });

    // スライドタイトルの更新(`set_frame_title`)
    group.bench_function("set_frame_title", |b| {
        b.iter(|| {
            let parsed = parse(black_box(&doc));
            let editor = parsed.editor();
            let result =
                editor.set_frame_title(black_box(42), black_box("Updated Benchmark Title"));
            black_box(result).expect("スライドタイトルの更新に失敗。");
        });
    });

    group.finish();
}

criterion_group!(benches, bench_cold_parse, bench_edit_operations_100_slides);
criterion_main!(benches);
