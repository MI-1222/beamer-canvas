//! `TeX` コンパイルログの構造化解析を行うモジュール。
//!
//! `TeX` エンジン特有の79文字強制折り返しの結合処理、
//! エラー発生行、警告、不足ファイルの抽出などを提供する。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 診断情報の重大度を表す列挙型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    /// エラー(コンパイル失敗や構文エラー)。
    Error,
    /// 警告(未解決の参照やボックスのはみ出しなど)。
    Warning,
    /// 情報(詳細な処理通知など)。
    Info,
}

/// `TeX` ログから抽出された1件の構造化診断情報。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// 重大度。
    pub severity: DiagnosticSeverity,
    /// エラーまたは警告のメッセージ。
    pub message: String,
    /// 発生元のファイルパス。
    pub file: Option<PathBuf>,
    /// 発生元の行番号(1始まり)。
    pub line: Option<usize>,
}

/// ログ全体の解析結果を保持するサマリー構造体。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TexLogSummary {
    /// 抽出された診断情報の一覧。
    pub diagnostics: Vec<Diagnostic>,
    /// ログから検知された不足ファイル名の一覧。
    pub missing_files: Vec<String>,
}

impl TexLogSummary {
    /// エラーレベルの診断情報が存在するかどうかを判定する。
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error)
    }

    /// 警告レベルの診断情報が存在するかどうかを判定する。
    pub fn has_warnings(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Warning)
    }
}

/// `TeX` ログ解析器。
#[derive(Debug, Default, Clone)]
pub struct TexLogParser;

impl TexLogParser {
    /// 新しい解析器インスタンスを生成する。
    pub fn new() -> Self {
        Self
    }

    /// `TeX` ログ文字列を行単位に分割し、強制折り返し(79〜80文字)を結合して正規化する。
    pub fn unwrap_lines(raw_log: &str) -> Vec<String> {
        let lines: Vec<&str> = raw_log.lines().collect();
        let mut unwrapped: Vec<String> = Vec::new();

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];

            // 79文字または80文字で改行されている場合は次行との結合を検討する。
            if (line.len() == 79 || line.len() == 80) && i + 1 < lines.len() {
                let next_line = lines[i + 1];
                // 次行が独立した構文開始行(エラー行、行番号、空行など)でない場合は結合する。
                if !next_line.is_empty()
                    && !next_line.starts_with('!')
                    && !next_line.starts_with("l.")
                    && !next_line.starts_with("LaTeX Warning:")
                    && !next_line.starts_with("Package ")
                {
                    unwrapped.push(format!("{}{}", line, next_line));
                    i += 2;
                    continue;
                }
            }

            unwrapped.push(line.to_string());
            i += 1;
        }

        unwrapped
    }

    /// ログ文字列全体を走査して診断サマリーを生成する。
    pub fn parse(&self, raw_log: &str) -> TexLogSummary {
        let lines = Self::unwrap_lines(raw_log);
        let mut diagnostics = Vec::new();
        let mut missing_files = Vec::new();

        let len = lines.len();
        let mut i = 0;

        while i < len {
            let line = &lines[i];

            // 1. 致命的エラー行の検知(`! ` で始まる行)。
            if let Some(err_msg) = line.strip_prefix("! ") {
                let mut message = err_msg.trim().to_string();
                let mut line_number = None;
                let file_path = None;

                // 不足ファイルの抽出(例: ! LaTeX Error: File 'foo.png' not found.)。
                if let Some(filename) =
                    Self::extract_missing_filename(&message).filter(|f| !missing_files.contains(f))
                {
                    missing_files.push(filename);
                }

                // 後続行から発生行番号(`l.<番号>`)や追加コンテキストを探索する。
                let mut lookahead = i + 1;
                while lookahead < len && lookahead <= i + 5 {
                    let next = lines[lookahead].trim();

                    if next.starts_with("l.") {
                        if let Some((num, rest)) = Self::extract_line_number_and_code(next) {
                            line_number = Some(num);
                            if !rest.is_empty() {
                                message.push_str(&format!(" (at line {}: {})", num, rest));
                            }
                        }
                        break;
                    }

                    // 次の独立エラー行に達した場合は走査を打ち切る。
                    if next.starts_with('!') {
                        break;
                    }

                    lookahead += 1;
                }

                diagnostics.push(Diagnostic {
                    severity: DiagnosticSeverity::Error,
                    message,
                    file: file_path,
                    line: line_number,
                });

                i = lookahead.max(i + 1);
                continue;
            }

            // 2. 警告行の検知(`LaTeX Warning:` や `Package <名前> Warning:`)。
            if line.contains("Warning:") {
                let severity = DiagnosticSeverity::Warning;
                let message = line.trim().to_string();
                let line_number = Self::extract_line_from_warning(line).or_else(|| {
                    // 次行に `on input line XX.` が続く場合に対応する。
                    if i + 1 < len {
                        Self::extract_line_from_warning(&lines[i + 1])
                    } else {
                        None
                    }
                });

                diagnostics.push(Diagnostic {
                    severity,
                    message,
                    file: None,
                    line: line_number,
                });

                i += 1;
                continue;
            }

            // 3. ボックスのはみ出しや不足警告(`Overfull` / `Underfull`)。
            if line.starts_with("Overfull \\hbox")
                || line.starts_with("Underfull \\hbox")
                || line.starts_with("Overfull \\vbox")
                || line.starts_with("Underfull \\vbox")
            {
                let severity = DiagnosticSeverity::Warning;
                let message = line.trim().to_string();
                let line_number = Self::extract_box_line_number(line);

                diagnostics.push(Diagnostic {
                    severity,
                    message,
                    file: None,
                    line: line_number,
                });

                i += 1;
                continue;
            }

            i += 1;
        }

        TexLogSummary {
            diagnostics,
            missing_files,
        }
    }

    /// エラーメッセージから不足ファイル名を抽出する。
    fn extract_missing_filename(message: &str) -> Option<String> {
        // パターン例: File `test.png' not found. や File 'test.png' not found
        let lower = message.to_lowercase();
        if !lower.contains("file") || !lower.contains("not found") {
            return None;
        }

        let start_bytes = message.as_bytes();
        let mut start_idx = None;

        for (idx, &b) in start_bytes.iter().enumerate() {
            if b == b'`' || b == b'\'' || b == b'"' {
                if start_idx.is_none() {
                    start_idx = Some(idx + 1);
                } else if let Some(start) = start_idx.filter(|&start| idx > start) {
                    return Some(message[start..idx].to_string());
                }
            }
        }

        None
    }

    /// `l.<数字> <コード>` 形式の行から行番号とコード片を抽出する。
    fn extract_line_number_and_code(line: &str) -> Option<(usize, String)> {
        let trimmed = line.trim();
        if !trimmed.starts_with("l.") {
            return None;
        }

        let after_prefix = &trimmed[2..];
        let mut digits = String::new();
        let mut chars = after_prefix.chars();

        for ch in chars.by_ref() {
            if ch.is_ascii_digit() {
                digits.push(ch);
            } else {
                break;
            }
        }

        if digits.is_empty() {
            return None;
        }

        let line_num = digits.parse::<usize>().ok()?;
        let rest: String = chars.collect();
        Some((line_num, rest.trim().to_string()))
    }

    /// 警告メッセージ内の行番号(`line 42` や `lines 10--20` など)を抽出する。
    fn extract_line_from_warning(text: &str) -> Option<usize> {
        let patterns = ["on input line ", "line "];
        for pattern in patterns {
            if let Some(pos) = text.find(pattern) {
                let after = &text[pos + pattern.len()..];
                let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(num) = digits.parse::<usize>() {
                    return Some(num);
                }
            }
        }
        None
    }

    /// `Overfull` または `Underfull` ボックス警告の `lines XX--YY` や `line XX` から開始行番号を抽出する。
    fn extract_box_line_number(text: &str) -> Option<usize> {
        if let Some(pos) = text.find("at lines ") {
            let after = &text[pos + "at lines ".len()..];
            let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            return digits.parse::<usize>().ok();
        }

        if let Some(pos) = text.find("at line ") {
            let after = &text[pos + "at line ".len()..];
            let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            return digits.parse::<usize>().ok();
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_undefined_control_sequence_parsing() {
        let raw_log = r#"
! Undefined control sequence.
l.15 \invalidmacro
                  {argument}
The control sequence at the end of the top line
"#;
        let parser = TexLogParser::new();
        let summary = parser.parse(raw_log);

        assert!(summary.has_errors());
        assert_eq!(summary.diagnostics.len(), 1);

        let diag = &summary.diagnostics[0];
        assert_eq!(diag.severity, DiagnosticSeverity::Error);
        assert_eq!(diag.line, Some(15));
        assert!(diag.message.contains("Undefined control sequence"));
        assert!(diag.message.contains("invalidmacro"));
    }

    #[test]
    fn test_missing_file_parsing() {
        let raw_log = r#"
! LaTeX Error: File `nonexistent.png' not found.

See the LaTeX manual or LaTeX Companion for explanation.
Type  H <return>  for immediate help.
 ...                                              
l.24 \includegraphics{nonexistent.png}
"#;
        let parser = TexLogParser::new();
        let summary = parser.parse(raw_log);

        assert!(summary.has_errors());
        assert_eq!(summary.missing_files, vec!["nonexistent.png"]);

        let diag = &summary.diagnostics[0];
        assert_eq!(diag.line, Some(24));
        assert!(diag.message.contains("File `nonexistent.png' not found"));
    }

    #[test]
    fn test_warning_parsing() {
        let raw_log = r#"
LaTeX Warning: Reference `sec:intro' on page 1 undefined on input line 42.
Package hyperref Warning: Token not allowed in a PDF string on input line 108.
"#;
        let parser = TexLogParser::new();
        let summary = parser.parse(raw_log);

        assert!(!summary.has_errors());
        assert!(summary.has_warnings());
        assert_eq!(summary.diagnostics.len(), 2);

        assert_eq!(summary.diagnostics[0].severity, DiagnosticSeverity::Warning);
        assert_eq!(summary.diagnostics[0].line, Some(42));

        assert_eq!(summary.diagnostics[1].severity, DiagnosticSeverity::Warning);
        assert_eq!(summary.diagnostics[1].line, Some(108));
    }

    #[test]
    fn test_overfull_box_warning() {
        let raw_log = r#"
Overfull \hbox (15.42pt too wide) in paragraph at lines 50--55
"#;
        let parser = TexLogParser::new();
        let summary = parser.parse(raw_log);

        assert!(summary.has_warnings());
        assert_eq!(summary.diagnostics.len(), 1);
        assert_eq!(summary.diagnostics[0].line, Some(50));
    }

    #[test]
    fn test_unwrap_79_column_lines() {
        // 79文字の行とその継続行。
        let line1 =
            "This is a very long line that exceeds the standard terminal wrapping width 1234";
        assert_eq!(line1.len(), 79);
        let line2 = "5678 and continues here.";

        let raw_log = format!("{}\n{}", line1, line2);
        let unwrapped = TexLogParser::unwrap_lines(&raw_log);

        assert_eq!(unwrapped.len(), 1);
        assert_eq!(unwrapped[0], format!("{}{}", line1, line2));
    }
}
