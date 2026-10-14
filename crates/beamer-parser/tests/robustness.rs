//! エッジケースおよび異常構文に対する堅牢性検証テスト。
//!
//! タイピング途中の未完成構文、括弧の不整合、ランダム変異入力、
//! 極端に深いネストや巨大入力に対して `panic` を起こさず、
//! テキストの完全性を維持して処理を完走することを検証する。

use beamer_parser::parse;
use proptest::prelude::*;
use std::panic::{self, AssertUnwindSafe};

/// `panic` 捕捉とラウンドトリップ不変条件のアサーションを行うヘルパー関数。
fn assert_parser_robustness(input: &str) {
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let parsed = parse(input);
        let reconstructed = parsed.to_string();
        (parsed, reconstructed)
    }));

    match result {
        Ok((parsed, reconstructed)) => {
            assert_eq!(
                reconstructed, input,
                "構文エラーを含む入力でテキストの欠落が原因で失敗しました。\n入力: {:?}",
                input
            );
            // エラーが存在してもドキュメントルートが取得可能であることを確認する。
            let _ = parsed.syntax();
        }
        Err(err) => {
            panic!(
                "パーサーのパニック発生が原因で失敗しました。\n入力: {:?}\nエラー詳細: {:?}",
                input, err
            );
        }
    }
}

#[test]
fn test_typing_in_progress_incomplete_syntax() {
    let cases = [
        // 環境名のタイピング途中
        "\\begin{fra",
        "\\begin{frame",
        "\\begin{",
        "\\begin",
        "\\end{fra",
        "\\end{frame",
        "\\end{",
        "\\end",
        // オプションや引数のタイピング途中
        "\\begin{frame}[",
        "\\begin{frame}[plain",
        "\\begin{frame}[fragile,t",
        "\\begin{frame}<",
        "\\begin{frame}<1-",
        "\\begin{frame}{タイトル",
        "\\begin{frame}{タイトル}{サブタイトル",
        "\\begin{frame}{タイトル}\n本文\n",
        // ブロックや段組みの未完構文
        "\\begin{frame}\n\\begin{block}{ブロック見出し",
        "\\begin{frame}\n\\begin{columns}\n\\begin{column}{0.5\\textwidth",
        "\\begin{frame}\n\\begin{itemize}\n\\item 箇条書き1\n\\item",
        // コマンド引数の未完
        "\\begin{frame}\n\\includegraphics[width=\n\\end{frame}",
        "\\begin{frame}\n\\includegraphics{path/to/\n\\end{frame}",
        "\\begin{frame}\n\\vspace{\n\\end{frame}",
    ];

    for case in cases {
        assert_parser_robustness(case);
    }
}

#[test]
fn test_unbalanced_and_crossed_environments() {
    let cases = [
        // 閉じタグが欠落したまま `EOF`
        "\\documentclass{beamer}\n\\begin{document}\n\\begin{frame}\n本文のみでendなし",
        // 内部環境が開いたまま外側環境が閉じる
        "\\begin{frame}\n\\begin{block}{見出し}\n閉じていないブロック\n\\end{frame}",
        "\\begin{frame}\n\\begin{itemize}\n\\item 項目\n\\end{frame}",
        "\\begin{frame}\n\\begin{columns}\n\\begin{column}{0.5\\textwidth}\nテキスト\n\\end{frame}",
        // 閉じタグの環境名が不一致
        "\\begin{frame}\n\\end{itemize}",
        "\\begin{block}{タイトル}\n\\end{frame}",
    ];

    for case in cases {
        assert_parser_robustness(case);
    }
}

#[test]
fn test_isolated_special_characters_and_escapes() {
    let cases = [
        // 末尾の単独バックスラッシュ
        "\\",
        "\\\\\\",
        "\\begin{frame}\\",
        // 孤立した括弧
        "}",
        "{{}}}}}",
        "]",
        "[[]]]",
        ">",
        "<<>>",
        // 特殊文字
        "$",
        "$$$",
        "%",
        "%%%%",
        "&",
        "_",
        "^",
        "~",
        "#",
        // 複合的な異常シーケンス
        "\\{\\}\\[\\]\\%\\$\\&\\_\\^",
        "\\begin{frame}{$x = 10 % inline comment\n\\end{frame}",
    ];

    for case in cases {
        assert_parser_robustness(case);
    }
}

#[test]
fn test_deeply_nested_structures() {
    // 1000重の中括弧ネスト
    let open_braces = "{".repeat(1000);
    let close_braces = "}".repeat(1000);
    let deep_braces = format!("\\begin{{frame}}{open_braces}nested{close_braces}\\end{{frame}}");
    assert_parser_robustness(&deep_braces);

    // 中括弧の数が不揃いな深いネスト
    let unbalanced_deep = format!("\\begin{{frame}}{open_braces}unbalanced\\end{{frame}}");
    assert_parser_robustness(&unbalanced_deep);

    // 100重のブロックネスト
    let mut nested_blocks = String::from("\\begin{frame}\n");
    for i in 0..100 {
        nested_blocks.push_str(&format!("\\begin{{block}}{{Level {i}}}\n"));
    }
    nested_blocks.push_str("Deepest content\n");
    for _ in 0..100 {
        nested_blocks.push_str("\\end{block}\n");
    }
    nested_blocks.push_str("\\end{frame}\n");
    assert_parser_robustness(&nested_blocks);
}

#[test]
fn test_large_tex_document_resilience() {
    // 3000行規模の繰り返しスライドドキュメント
    let mut large_doc = String::from("\\documentclass{beamer}\n\\begin{document}\n");
    for i in 0..300 {
        large_doc.push_str(&format!(
            "\\begin{{frame}}{{スライド番号 {i}}}\n  \\begin{{itemize}}\n    \\item 項目 1 for slide {i}\n    \\item 項目 2 for slide {i}\n  \\end{{itemize}}\n\\end{{frame}}\n\n"
        ));
    }
    large_doc.push_str("\\end{document}\n");

    assert_parser_robustness(&large_doc);
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1000,
        .. ProptestConfig::default()
    })]

    #[test]
    fn test_property_based_random_mutation_fuzzing(
        seed in "\\PC*",
        mutation_ops in prop::collection::vec((0..5u8, 0..100usize), 1..20)
    ) {
        let base_template = r#"\documentclass{beamer}
\usetheme{Madrid}
\title{Robustness Test}
\begin{document}
\begin{frame}{Slide Title}
  \begin{block}{Block Title}
    Sample text with math $x + y = z$.
    \begin{itemize}
      \item Item 1
      \item Item 2
    \end{itemize}
  \end{block}
\end{frame}
\end{document}
"#;

        let mut mutated = base_template.to_string();

        for (op, pos_seed) in mutation_ops {
            if mutated.is_empty() {
                mutated.push_str("sample");
            }
            let char_count = mutated.chars().count();
            let target_char_idx = pos_seed % char_count;
            let byte_idx = mutated.char_indices().nth(target_char_idx).map(|(idx, _)| idx).unwrap_or(0);

            match op {
                0 => {
                    // 文字削除
                    if byte_idx < mutated.len() {
                        let ch = mutated[byte_idx..].chars().next().unwrap();
                        let ch_len = ch.len_utf8();
                        mutated.replace_range(byte_idx..byte_idx + ch_len, "");
                    }
                }
                1 => {
                    // バックスラッシュ挿入
                    mutated.insert(byte_idx, '\\');
                }
                2 => {
                    // 中括弧の挿入
                    let brace = if pos_seed % 2 == 0 { '{' } else { '}' };
                    mutated.insert(byte_idx, brace);
                }
                3 => {
                    // パーセント記号(コメント)挿入
                    mutated.insert(byte_idx, '%');
                }
                4 => {
                    // ランダム文字列の断片挿入
                    let fragment: String = seed.chars().take(5).collect();
                    mutated.insert_str(byte_idx, &fragment);
                }
                _ => {}
            }
        }

        assert_parser_robustness(&mutated);
    }
}
