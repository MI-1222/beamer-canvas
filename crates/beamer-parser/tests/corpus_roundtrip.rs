//! 実世界コーパスを用いたラウンドトリップ完全一致および可逆編集テスト。
//!
//! 多様な `Beamer` スライドのフィクスチャを走査し、
//! `parse` 後の再文字列化におけるバイト完全一致と編集操作の可逆性を検証する。

use beamer_parser::parse;
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::path::{Path, PathBuf};

/// 指定したディレクトリ配下の全 `.tex` ファイルのパスを再帰的に収集する。
fn collect_tex_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !dir.exists() {
        return files;
    }

    let entries = fs::read_dir(dir).expect("フィクスチャディレクトリの走査に失敗。");
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_tex_files(&path));
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("tex") {
            files.push(path);
        }
    }
    files.sort();
    files
}

/// 差分を視覚的にフォーマットしてエラーメッセージを構築する。
fn format_diff(expected: &str, actual: &str) -> String {
    let diff = TextDiff::from_lines(expected, actual);
    let mut output = String::new();
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => "-",
            ChangeTag::Insert => "+",
            ChangeTag::Equal => " ",
        };
        output.push_str(&format!("{sign} {change}"));
    }
    output
}

#[test]
fn test_corpus_fixtures_lossless_roundtrip() {
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let tex_files = collect_tex_files(&fixtures_dir);

    assert!(
        !tex_files.is_empty(),
        "テスト対象のフィクスチャファイルが存在しないため失敗。",
    );

    for path in &tex_files {
        let source = fs::read_to_string(path)
            .unwrap_or_else(|err| panic!("ファイル読み込みに失敗: {:?}, 原因: {err}", path));

        let parse_result = parse(&source);
        let reconstructed = parse_result.to_string();

        if source != reconstructed {
            let diff_report = format_diff(&source, &reconstructed);
            panic!(
                "フィクスチャのラウンドトリップ不一致が原因で失敗しました。\nファイル: {:?}\n差分:\n{}",
                path, diff_report
            );
        }
    }
}

#[test]
fn test_corpus_edit_reversibility() {
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let tex_files = collect_tex_files(&fixtures_dir);

    for path in &tex_files {
        let source = fs::read_to_string(path)
            .unwrap_or_else(|err| panic!("ファイル読み込みに失敗: {:?}, 原因: {err}", path));

        let parse_result = parse(&source);
        let editor = parse_result.editor();
        let frames_res = editor.get_frames();

        if let Ok(frames) = frames_res {
            // 2枚以上のスライドがある場合、`swap_frames` および `move_frame` の可逆性を検証する。
            if frames.len() >= 2 {
                // 1. `swap_frames` の可逆性検証
                let swapped = editor
                    .swap_frames(0, 1)
                    .expect("スライドのスワップに失敗。");
                let restored = swapped
                    .swap_frames(0, 1)
                    .expect("スライドの逆スワップに失敗。");
                assert_eq!(
                    source,
                    restored.text(),
                    "スライドスワップの往復操作で元ソースとの不一致が原因で失敗しました: {:?}",
                    path
                );

                // 2. `move_frame` の可逆性検証
                let moved = editor.move_frame(0, 1).expect("スライドの移動に失敗。");
                let restored_move = moved.move_frame(1, 0).expect("スライドの逆移動に失敗。");
                assert_eq!(
                    source,
                    restored_move.text(),
                    "スライド移動の往復操作で元ソースとの不一致が原因で失敗しました: {:?}",
                    path
                );
            }

            // 1枚以上のスライドがある場合、`set_frame_title` による編集と復元の可逆性を検証する。
            if !frames.is_empty()
                && let Some(original_title) = frames[0].title_text()
            {
                let temporary_title = "一時的な置換タイトル";
                let modified = editor
                    .set_frame_title(0, temporary_title)
                    .expect("タイトル更新に失敗。");
                let restored = modified
                    .set_frame_title(0, &original_title)
                    .expect("タイトル復元に失敗。");

                assert_eq!(
                    source,
                    restored.text(),
                    "タイトル変更・復元の往復操作で元ソースとの不一致が原因で失敗しました: {:?}",
                    path
                );
            }
        }
    }
}

#[test]
fn test_crlf_and_lf_mixed_roundtrip() {
    let base_content = "\\documentclass{beamer}\n\\begin{document}\n\\begin{frame}{Slide 1}\nContent\n\\end{frame}\n\\end{document}\n";
    let crlf_content = base_content.replace('\n', "\r\n");

    let parsed_lf = parse(base_content);
    assert_eq!(parsed_lf.to_string(), base_content);

    let parsed_crlf = parse(&crlf_content);
    assert_eq!(parsed_crlf.to_string(), crlf_content);

    // `CRLF` と `LF` の混在ケース
    let mixed_content = "\\documentclass{beamer}\r\n\\begin{document}\n\\begin{frame}{Slide 1}\r\nContent line 1\nContent line 2\r\n\\end{frame}\n\\end{document}\r\n";
    let parsed_mixed = parse(mixed_content);
    assert_eq!(parsed_mixed.to_string(), mixed_content);
}
