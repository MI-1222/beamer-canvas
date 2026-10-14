//! `beamer-verify`: コア基盤総合検証および品質判定(`Gate Criteria`)`CLI` ツール。
//!
//! 実世界コーパスを用いたラウンドトリップ完全一致度、パースクラッシュ耐性(ファジング)、
//! レイテンシ・プロファイル(パース・ビルド・画像化・キャッシュ)、および
//! ページマッピング整合性を自動計測し、総合評価レポートを出力する。

mod metrics;
mod report;

use clap::{Parser, Subcommand};
use metrics::fuzz::verify_fuzz_resilience;
use metrics::latency::verify_latency_profile;
use metrics::mapping::verify_page_mapping;
use metrics::roundtrip::verify_roundtrip;
use report::{GateStatus, VerificationReport};
use std::fs;
use std::path::PathBuf;

/// `BeamerCanvas` コア基盤総合検証CLIツール。
#[derive(Parser, Debug)]
#[command(name = "beamer-verify")]
#[command(about = "LaTeX Beamer コア基盤の品質検証およびGate Criteria判定ツール", long_about = None)]
struct Cli {
    /// テストコーパスのルートディレクトリ。
    #[arg(short, long, default_value = "crates/beamer-parser/tests/fixtures")]
    corpus_dir: PathBuf,

    /// ファジング実行イテレーション数。
    #[arg(short, long, default_value_t = 10_000)]
    fuzz_iterations: usize,

    /// 結果をJSON形式で標準出力に出力するフラグ。
    #[arg(long)]
    json: bool,

    /// 結果を指定のJSONファイルに書き出すパス。
    #[arg(long)]
    output_json: Option<PathBuf>,

    /// Markdown形式の要約表を出力するフラグ。
    #[arg(long, default_value_t = true)]
    markdown: bool,

    /// 実行するサブコマンド(省略時は全パイプライン実行)。
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 全パイプライン(ラウンドトリップ、ファジング、レイテンシ、マッピング)を一括実行。
    All,
    /// ラウンドトリップ完全一致度のみを検証。
    Roundtrip,
    /// パースクラッシュ耐性(ファジング)のみを検証。
    Fuzz,
    /// レイテンシ・プロファイルのみを計測。
    Latency,
    /// ページマッピング整合性のみを検証。
    Mapping,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let corpus_path = if cli.corpus_dir.exists() {
        cli.corpus_dir.clone()
    } else {
        // カレントディレクトリの差異を吸収(ワークスペースルートからの相対パス)。
        let fallback = PathBuf::from("crates/beamer-parser/tests/fixtures");
        if fallback.exists() {
            fallback
        } else if PathBuf::from("../../crates/beamer-parser/tests/fixtures").exists() {
            PathBuf::from("../../crates/beamer-parser/tests/fixtures")
        } else {
            PathBuf::from("../beamer-parser/tests/fixtures")
        }
    };

    let command = cli.command.unwrap_or(Commands::All);

    match command {
        Commands::All => {
            run_all_verifications(
                &corpus_path,
                cli.fuzz_iterations,
                cli.json,
                cli.output_json,
                cli.markdown,
            )
            .await;
        }
        Commands::Roundtrip => {
            let report = verify_roundtrip(&corpus_path);
            println!(
                "ラウンドトリップ一致度: {:.2}% ({}/{}) -> [{}]",
                report.match_rate,
                report.matched_files,
                report.total_files,
                report.status.as_str()
            );
            if report.status == GateStatus::Fail {
                std::process::exit(1);
            }
        }
        Commands::Fuzz => {
            let report = verify_fuzz_resilience(cli.fuzz_iterations);
            println!(
                "ファジングパニック率: {:.2}% ({} 回実行) -> [{}]",
                report.panic_rate,
                report.total_iterations,
                report.status.as_str()
            );
            if report.status == GateStatus::Fail {
                std::process::exit(1);
            }
        }
        Commands::Latency => {
            let report = verify_latency_profile().await;
            println!(
                "パース速度 (100スライド): {:.2} ms -> [{}]",
                report.parse_time_100_slides_micros as f64 / 1000.0,
                report.parse_status.as_str()
            );
            if let Some(ms) = report.compile_partial_millis {
                println!(
                    "部分コンパイル速度: {} ms -> [{}]",
                    ms,
                    report.compile_partial_status.as_str()
                );
            }
            println!(
                "キャッシュ取得速度: L1 = {} µs, L2 = {:.2} ms -> [{}]",
                report.cache_l1_micros,
                report.cache_l2_millis,
                report.cache_status.as_str()
            );
        }
        Commands::Mapping => {
            if let Some(report) = verify_page_mapping().await {
                println!(
                    "マッピング正解率: {:.2}% ({}/{} フレーム) -> [{}]",
                    report.accuracy_rate,
                    report.matched_frames,
                    report.total_frames,
                    report.status.as_str()
                );
                if report.status == GateStatus::Fail {
                    std::process::exit(1);
                }
            } else {
                println!("環境要件が未達のため、マッピング検証をスキップしました。");
            }
        }
    }
}

/// 全検証パイプラインを実行して総合レポートを出力する。
async fn run_all_verifications(
    corpus_dir: &PathBuf,
    fuzz_iterations: usize,
    output_json_stdout: bool,
    output_json_path: Option<PathBuf>,
    show_markdown: bool,
) {
    println!("検証パイプラインの実行を開始中... (対象コーパス: {:?})", corpus_dir);

    // 1. ラウンドトリップ検証。
    let roundtrip_rep = verify_roundtrip(corpus_dir);

    // 2. ファジング耐性検証。
    let fuzz_rep = verify_fuzz_resilience(fuzz_iterations);

    // 3. レイテンシプロファイル。
    let latency_rep = verify_latency_profile().await;

    // 4. ページマッピング検証。
    let mapping_rep = verify_page_mapping().await;

    let mut verification = VerificationReport {
        environment_info: format!(
            "OS: {} ({}) / Target: {}",
            std::env::consts::OS,
            std::env::consts::ARCH,
            std::env::var("TARGET").unwrap_or_else(|_| "native".to_string())
        ),
        roundtrip: roundtrip_rep,
        fuzz: fuzz_rep,
        latency: latency_rep,
        mapping: mapping_rep,
        overall_status: GateStatus::Pass,
    };

    verification.compute_overall_status();

    // 出力処理。
    if output_json_stdout {
        if let Ok(json_str) = serde_json::to_string_pretty(&verification) {
            println!("{json_str}");
        }
    } else {
        verification.print_summary();

        if show_markdown {
            println!("\n### Markdown 要約表\n");
            println!("{}", verification.to_markdown_table());
        }
    }

    if let Some(ref path) = output_json_path {
        if let Ok(json_str) = serde_json::to_string_pretty(&verification) {
            let _ = fs::write(path, json_str);
            println!("レポートファイルの出力に成功: {:?}", path);
        }
    }

    if verification.overall_status == GateStatus::Fail {
        std::process::exit(1);
    }
}
