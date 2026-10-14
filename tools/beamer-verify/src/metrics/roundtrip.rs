//! ラウンドトリップ完全一致度(`Lossless Fidelity`)の検証モジュール。
//!
//! コーパス内の全 `TeX` ファイルを走査し、`parse(S).to_string() == S` の
//! バイト単位厳密比較を実行して完全一致率を算出する。

use crate::report::{GateStatus, RoundtripReport};
use beamer_parser::parse;
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::path::{Path, PathBuf};

/// 指定ディレクトリ配下の全 `.tex` ファイルを再帰的に収集する。
pub fn collect_tex_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !dir.exists() {
        return files;
    }

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_tex_files(&path));
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("tex") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// 差分を文字列として整形する。
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

/// コーパスディレクトリに対してラウンドトリップ検証を実行する。
pub fn verify_roundtrip(corpus_dir: &Path) -> RoundtripReport {
    let tex_files = collect_tex_files(corpus_dir);
    let total_files = tex_files.len();

    if total_files == 0 {
        return RoundtripReport {
            total_files: 0,
            matched_files: 0,
            match_rate: 0.0,
            mismatches: vec!["検証対象となるTeXファイルの検出に失敗。".to_string()],
            status: GateStatus::Fail,
        };
    }

    let mut matched_files = 0;
    let mut mismatches = Vec::new();

    for path in &tex_files {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(err) => {
                mismatches.push(format!("{path:?}: ファイルの読み込みに失敗: {err}"));
                continue;
            }
        };

        let parsed = parse(&content);
        let reconstructed = parsed.to_string();

        if content == reconstructed {
            matched_files += 1;
        } else {
            let diff = format_diff(&content, &reconstructed);
            mismatches.push(format!("{path:?}: ラウンドトリップ比較の検証に失敗。\n{diff}"));
        }
    }

    let match_rate = (matched_files as f64 / total_files as f64) * 100.0;
    let status = if match_rate >= 100.0 {
        GateStatus::Pass
    } else if match_rate >= 99.9 {
        GateStatus::Warning
    } else {
        GateStatus::Fail
    };

    RoundtripReport {
        total_files,
        matched_files,
        match_rate,
        mismatches,
        status,
    }
}
