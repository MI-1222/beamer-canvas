//! パースクラッシュ耐性(`Panic Resilience`)の検証モジュール。
//!
//! ランダム変異や構文破壊を施した入力を大量にパーサーに流し込み、
//! `catch_unwind` を用いてパニックの非発生を検証する。

use crate::report::{FuzzReport, GateStatus};
use beamer_parser::{SyntaxKind, parse};
use rand::RngExt;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::panic;

/// 基本となる健全な `Beamer` スニペット。
const BASE_BEAMER_SOURCE: &str = r#"\documentclass{beamer}
\usetheme{Madrid}
\begin{document}
\begin{frame}<1-2>{Fuzzing Verification Slide}
  \framesubtitle{Robustness against arbitrary broken tokens}
  \begin{block}{Theoretical Context}
    Formula evaluation: $E = mc^2$ and $\sum_{i=1}^n x_i$.
  \end{block}
  \begin{itemize}
    \item Item number one with \alert{emphasis}.
    \item \textbf{Bold text} before pause.
  \end{itemize}
  \pause
  Second step reveals additional text.
\end{frame}
\end{document}
"#;

/// 文字列に対してランダムな変異を加える。
fn mutate_source(source: &str, rng: &mut StdRng) -> String {
    let mutation_type = rng.random_range(0..6);
    let mut chars: Vec<char> = source.chars().collect();
    let len = chars.len();

    if len == 0 {
        return "\\begin{frame}".to_string();
    }

    match mutation_type {
        0 => {
            // ランダム位置への切り詰め(`Truncation`)。
            let cut_point = rng.random_range(0..len);
            chars.truncate(cut_point);
        }
        1 => {
            // ランダム位置への特殊記号の挿入。
            let insert_symbols = ['\\', '{', '}', '[', ']', '<', '>', '$', '%', '&', '_', '^'];
            let symbol = insert_symbols[rng.random_range(0..insert_symbols.len())];
            let pos = rng.random_range(0..=len);
            chars.insert(pos, symbol);
        }
        2 => {
            // ランダム区間の文字削除。
            let start = rng.random_range(0..len);
            let delete_len = rng.random_range(1..=(len - start).min(20));
            chars.drain(start..start + delete_len);
        }
        3 => {
            // ランダム位置の文字の置換。
            let pos = rng.random_range(0..len);
            let random_char = match rng.random_range(0..3) {
                0 => '\\',
                1 => '}',
                _ => '$',
            };
            chars[pos] = random_char;
        }
        4 => {
            // 未完の環境タグの注入。
            let broken_tags = [
                "\\begin{frame",
                "\\end{",
                "\\begin{block}{",
                "\\begin{columns}[",
                "\\frametitle{",
                "\\item[<",
            ];
            let tag = broken_tags[rng.random_range(0..broken_tags.len())];
            let pos = rng.random_range(0..=len);
            for (idx, ch) in tag.chars().enumerate() {
                chars.insert(pos + idx, ch);
            }
        }
        _ => {
            // ランダムな制御文字やマルチバイト文字の混入。
            let pos = rng.random_range(0..=len);
            chars.insert(pos, 'あ');
            chars.insert(pos + 1, '🍣');
        }
    }

    chars.into_iter().collect()
}

/// ファジングによるパースクラッシュ耐性を検証する。
pub fn verify_fuzz_resilience(iterations: usize) -> FuzzReport {
    let mut rng = StdRng::seed_from_u64(42);
    let mut panic_count = 0;
    let mut error_nodes_count = 0;

    for _ in 0..iterations {
        let mutated = mutate_source(BASE_BEAMER_SOURCE, &mut rng);

        let parse_result = panic::catch_unwind(|| {
            let parsed = parse(&mutated);
            let mut err_count = 0;
            for node in parsed.syntax().descendants() {
                if node.kind() == SyntaxKind::Error {
                    err_count += 1;
                }
            }
            err_count
        });

        match parse_result {
            Ok(errors) => {
                error_nodes_count += errors;
            }
            Err(_) => {
                panic_count += 1;
            }
        }
    }

    let panic_rate = (panic_count as f64 / iterations as f64) * 100.0;
    let status = if panic_count == 0 {
        GateStatus::Pass
    } else {
        GateStatus::Fail
    };

    FuzzReport {
        total_iterations: iterations,
        panic_count,
        panic_rate,
        error_nodes_count,
        status,
    }
}
