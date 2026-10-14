//! 検証結果のデータ構造およびレポート生成モジュール。
//!
//! 収集した各メトリクスを評価基準である `Gate Criteria` と照合し、
//! コンソール表示、Markdown形式の要約表、およびJSON形式で出力する。

use serde::{Deserialize, Serialize};

/// 各メトリクスの合否判定ステータス。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateStatus {
    /// 合格基準を完全にクリア。
    Pass,
    /// 許容範囲内だが注意を要する状態。
    Warning,
    /// 基準未達であり修正が必要。
    Fail,
}

impl GateStatus {
    /// ステータスの表示用ラベルを取得する。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Warning => "WARN",
            Self::Fail => "FAIL",
        }
    }
}

/// ラウンドトリップ検証の結果詳細。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundtripReport {
    /// 検証対象となったファイルの総数。
    pub total_files: usize,
    /// バイト単位で完全一致したファイル数。
    pub matched_files: usize,
    /// 一致率(パーセンテージ)。
    pub match_rate: f64,
    /// 不一致となったファイルの詳細リスト。
    pub mismatches: Vec<String>,
    /// 判定ステータス。
    pub status: GateStatus,
}

/// パースクラッシュ耐性(ファジング)検証の結果詳細。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzReport {
    /// 実行した変異イテレーション数。
    pub total_iterations: usize,
    /// パニック発生回数。
    pub panic_count: usize,
    /// パニック発生率(百分率)。
    pub panic_rate: f64,
    /// 生成された構文エラーノードの総数。
    pub error_nodes_count: usize,
    /// 判定ステータス。
    pub status: GateStatus,
}

/// レイテンシ・プロファイル測定の結果詳細。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyReport {
    /// `100` スライド相当のパース時間(マイクロ秒)。
    pub parse_time_100_slides_micros: u64,
    /// 初回フルコンパイル時間(ミリ秒)。
    pub compile_full_millis: Option<u64>,
    /// 単一スライド部分コンパイル時間(ミリ秒)。
    pub compile_partial_millis: Option<u64>,
    /// `PDFium` による `150 DPI` ラスタライズ時間(ミリ秒)。
    pub raster_150dpi_millis: Option<f64>,
    /// `PDFium` による `300 DPI` ラスタライズ時間(ミリ秒)。
    pub raster_300dpi_millis: Option<f64>,
    /// `L1` メモリキャッシュの取得時間(マイクロ秒)。
    pub cache_l1_micros: u64,
    /// `L2` ディスクキャッシュの復元時間(ミリ秒)。
    pub cache_l2_millis: f64,
    /// コンパイルのバイパスによる高速化倍率。
    pub speedup_factor: Option<f64>,
    /// パース速度の判定ステータス。
    pub parse_status: GateStatus,
    /// 部分コンパイル速度の判定ステータス。
    pub compile_partial_status: GateStatus,
    /// キャッシュ取得速度の判定ステータス。
    pub cache_status: GateStatus,
}

/// ページマッピング検証の結果詳細。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingReport {
    /// 検証対象となったスライドおよびフレームの総数。
    pub total_frames: usize,
    /// 全体の物理ページ総数。
    pub total_pages: usize,
    /// 正しく対応付けられたフレーム数。
    pub matched_frames: usize,
    /// マッピング正解率(百分率)。
    pub accuracy_rate: f64,
    /// 判定ステータス。
    pub status: GateStatus,
}

/// 総合検証レポート。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    /// 実行日時および環境の識別文字列。
    pub environment_info: String,
    /// ラウンドトリップ検証レポート。
    pub roundtrip: RoundtripReport,
    /// ファジング耐性検証レポート。
    pub fuzz: FuzzReport,
    /// レイテンシ検証レポート。
    pub latency: LatencyReport,
    /// ページマッピング検証レポート。
    pub mapping: Option<MappingReport>,
    /// 全体総合判定。
    pub overall_status: GateStatus,
}

impl VerificationReport {
    /// 総合判定ステータスを算出する。
    pub fn compute_overall_status(&mut self) {
        let statuses = [
            self.roundtrip.status,
            self.fuzz.status,
            self.latency.parse_status,
            self.latency.compile_partial_status,
            self.latency.cache_status,
            self.mapping
                .as_ref()
                .map(|m| m.status)
                .unwrap_or(GateStatus::Pass),
        ];

        if statuses.contains(&GateStatus::Fail) {
            self.overall_status = GateStatus::Fail;
        } else if statuses.contains(&GateStatus::Warning) {
            self.overall_status = GateStatus::Warning;
        } else {
            self.overall_status = GateStatus::Pass;
        }
    }

    /// コンソール向けのテキスト要約を出力する。
    pub fn print_summary(&self) {
        println!("============================================================");
        println!("           beamer-verify 総合検証レポート");
        println!("============================================================");
        println!("環境情報: {}", self.environment_info);
        println!("総合判定: [{}]", self.overall_status.as_str());
        println!("------------------------------------------------------------");
        println!(
            "1. ラウンドトリップ完全一致度: {:.2}% ({}/{}) -> [{}]",
            self.roundtrip.match_rate,
            self.roundtrip.matched_files,
            self.roundtrip.total_files,
            self.roundtrip.status.as_str()
        );
        println!(
            "2. パースクラッシュ耐性: パニック率 {:.2}% ({} 回実行) -> [{}]",
            self.fuzz.panic_rate,
            self.fuzz.total_iterations,
            self.fuzz.status.as_str()
        );
        println!(
            "3. パース処理速度 (100スライド): {:.2} ms -> [{}]",
            self.latency.parse_time_100_slides_micros as f64 / 1000.0,
            self.latency.parse_status.as_str()
        );
        if let Some(partial_ms) = self.latency.compile_partial_millis {
            println!(
                "   部分コンパイル速度: {} ms -> [{}]",
                partial_ms,
                self.latency.compile_partial_status.as_str()
            );
        }
        println!(
            "   キャッシュ取得速度: L1 = {} µs, L2 = {:.2} ms -> [{}]",
            self.latency.cache_l1_micros,
            self.latency.cache_l2_millis,
            self.latency.cache_status.as_str()
        );
        if let Some(ref mapping) = self.mapping {
            println!(
                "4. ページマッピング正解率: {:.2}% ({}/{} フレーム) -> [{}]",
                mapping.accuracy_rate,
                mapping.matched_frames,
                mapping.total_frames,
                mapping.status.as_str()
            );
        }
        println!("============================================================");
    }

    /// Markdown形式の要約表を生成する。
    pub fn to_markdown_table(&self) -> String {
        let mut md = String::new();
        md.push_str("| 評価指標 | 実測値 | 目標合格基準 | 判定 |\n");
        md.push_str("| :--- | :--- | :--- | :--- |\n");

        md.push_str(&format!(
            "| **ラウンドトリップ一致度** | **{:.2}%** ({}/{} ファイル) | 100.00% バイト一致 | **{}** |\n",
            self.roundtrip.match_rate,
            self.roundtrip.matched_files,
            self.roundtrip.total_files,
            self.roundtrip.status.as_str()
        ));

        md.push_str(&format!(
            "| **パースクラッシュ耐性** | **パニック {} 件** (全 {} 回変異走査) | 0.00% (パニック0件) | **{}** |\n",
            self.fuzz.panic_count,
            self.fuzz.total_iterations,
            self.fuzz.status.as_str()
        ));

        md.push_str(&format!(
            "| **CSTパース速度 (100スライド)** | **約 {:.2} ms** ({} µs) | < 10.0 ms | **{}** |\n",
            self.latency.parse_time_100_slides_micros as f64 / 1000.0,
            self.latency.parse_time_100_slides_micros,
            self.latency.parse_status.as_str()
        ));

        if let Some(partial_ms) = self.latency.compile_partial_millis {
            let full_str = self
                .latency
                .compile_full_millis
                .map(|m| format!("{m} ms"))
                .unwrap_or_else(|| "N/A".to_string());
            let speedup_str = self
                .latency
                .speedup_factor
                .map(|f| format!("約 {f:.0} 倍高速"))
                .unwrap_or_default();
            md.push_str(&format!(
                "| **単一スライド部分コンパイル** | **{} ms** (フル: {}, {}) | < 1.0 s (1000 ms) | **{}** |\n",
                partial_ms,
                full_str,
                speedup_str,
                self.latency.compile_partial_status.as_str()
            ));
        }

        md.push_str(&format!(
            "| **キャッシュ取得レイテンシ** | **L1: {} µs** / **L2: {:.2} ms** | L1 < 2 ms / L2 < 15 ms | **{}** |\n",
            self.latency.cache_l1_micros,
            self.latency.cache_l2_millis,
            self.latency.cache_status.as_str()
        ));

        if let Some(ref mapping) = self.mapping {
            md.push_str(&format!(
                "| **ページマッピング正解率** | **{:.2}%** ({}/{} フレーム, 全{}ページ) | 100.00% 整合 | **{}** |\n",
                mapping.accuracy_rate,
                mapping.matched_frames,
                mapping.total_frames,
                mapping.total_pages,
                mapping.status.as_str()
            ));
        }

        md
    }
}
