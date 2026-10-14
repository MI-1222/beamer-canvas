//! システム上の `TeX` コマンドの存在確認およびバージョン自動検出モジュール。

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 検出対象となる主要な `TeX` コマンドの種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TexCommandKind {
    /// 自動リラン・依存関係解決ツール `latexmk`。
    Latexmk,
    /// `LuaTeX` ベースのエンジン `lualatex`。
    Lualatex,
    /// `XeTeX` ベースのエンジン `xelatex`。
    Xelatex,
    /// `pdfTeX` ベースのエンジン `pdflatex`。
    Pdflatex,
}

impl TexCommandKind {
    /// 全てのサポート対象コマンド種別をスライスで返す。
    pub const fn all() -> &'static [Self] {
        &[Self::Latexmk, Self::Lualatex, Self::Xelatex, Self::Pdflatex]
    }

    /// コマンドの実行可能ファイル名を返す。
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Latexmk => "latexmk",
            Self::Lualatex => "lualatex",
            Self::Xelatex => "xelatex",
            Self::Pdflatex => "pdflatex",
        }
    }

    /// `UI` 表示用の名称を返す。
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Latexmk => "Latexmk",
            Self::Lualatex => "LuaLaTeX",
            Self::Xelatex => "XeLaTeX",
            Self::Pdflatex => "pdfLaTeX",
        }
    }
}

impl fmt::Display for TexCommandKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// 検出された `TeX` コマンドのメタデータを保持する構造体。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TexToolInfo {
    /// コマンドの種類。
    pub kind: TexCommandKind,
    /// 実行可能ファイルの絶対パス。
    pub path: Option<PathBuf>,
    /// 抽出されたバージョン識別子(例: `"4.87"`, `"1.24.0"`)。
    pub version: String,
    /// 検出されたディストリビューション情報(例: `"TeX Live 2026/Homebrew"`)。
    pub distribution: Option<String>,
    /// `--version` 出力の先頭行テキスト。
    pub raw_version: String,
}

/// システム上に検出された `TeX` 環境全体を表す構造体。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TexEnvironment {
    /// 検出された各ツールの情報一覧。
    pub tools: Vec<TexToolInfo>,
}

impl TexEnvironment {
    /// システム上の `TeX` コマンドを並行して自動検出し、環境情報を構築する。
    pub async fn detect() -> Self {
        let mut tasks = Vec::new();
        for &kind in TexCommandKind::all() {
            tasks.push(tokio::spawn(async move { detect_tool(kind).await }));
        }

        let mut tools = Vec::new();
        for task in tasks {
            if let Ok(Some(info)) = task.await {
                tools.push(info);
            }
        }

        // コマンド種別の定義順序に合わせて整列する。
        tools.sort_by_key(|tool| {
            TexCommandKind::all()
                .iter()
                .position(|&k| k == tool.kind)
                .unwrap_or(usize::MAX)
        });

        Self { tools }
    }

    /// 指定したコマンドが利用可能であるか判定する。
    pub fn is_available(&self, kind: TexCommandKind) -> bool {
        self.tools.iter().any(|tool| tool.kind == kind)
    }

    /// 指定したコマンドの検出情報を取得する。
    pub fn get_tool(&self, kind: TexCommandKind) -> Option<&TexToolInfo> {
        self.tools.iter().find(|tool| tool.kind == kind)
    }

    /// `latexmk` の情報を取得する。
    pub fn latexmk(&self) -> Option<&TexToolInfo> {
        self.get_tool(TexCommandKind::Latexmk)
    }

    /// `lualatex` の情報を取得する。
    pub fn lualatex(&self) -> Option<&TexToolInfo> {
        self.get_tool(TexCommandKind::Lualatex)
    }

    /// `xelatex` の情報を取得する。
    pub fn xelatex(&self) -> Option<&TexToolInfo> {
        self.get_tool(TexCommandKind::Xelatex)
    }

    /// `pdflatex` の情報を取得する。
    pub fn pdflatex(&self) -> Option<&TexToolInfo> {
        self.get_tool(TexCommandKind::Pdflatex)
    }

    /// 推奨されるコンパイルエンジンを取得する。
    ///
    /// 自動リランが可能な `latexmk` を最優先とし、次いで `UTF-8` やモダンフォントに対応する `lualatex`、`xelatex`、`pdflatex` の順に選定する。
    pub fn preferred_engine(&self) -> Option<TexCommandKind> {
        const PREFERENCE: [TexCommandKind; 4] = [
            TexCommandKind::Latexmk,
            TexCommandKind::Lualatex,
            TexCommandKind::Xelatex,
            TexCommandKind::Pdflatex,
        ];
        PREFERENCE
            .iter()
            .find(|&&kind| self.is_available(kind))
            .copied()
    }
}

/// 単一の `TeX` コマンドを検出し、そのメタデータを取得する。
pub async fn detect_tool(kind: TexCommandKind) -> Option<TexToolInfo> {
    let command_name = kind.as_str();
    let path = find_executable_in_path(command_name);
    let executable_target = path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| command_name.to_string());

    let output = tokio::time::timeout(
        Duration::from_secs(4),
        tokio::process::Command::new(&executable_target)
            .arg("--version")
            .output(),
    )
    .await
    .ok()?
    .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let stderr_str = String::from_utf8_lossy(&output.stderr);

    let (version, distribution, raw_version) =
        parse_version_output(kind, &stdout_str, &stderr_str)?;

    Some(TexToolInfo {
        kind,
        path,
        version,
        distribution,
        raw_version,
    })
}

/// コマンドの出力テキストからバージョン番号、ディストリビューション情報、先頭行を抽出する。
pub fn parse_version_output(
    kind: TexCommandKind,
    stdout: &str,
    stderr: &str,
) -> Option<(String, Option<String>, String)> {
    let combined = if !stdout.trim().is_empty() {
        stdout
    } else {
        stderr
    };

    let first_line = combined
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?
        .to_string();

    let distribution = extract_distribution(&first_line).or_else(|| extract_distribution(combined));
    let version = extract_version_number(kind, &first_line)
        .or_else(|| extract_version_number(kind, combined))
        .unwrap_or_else(|| first_line.clone());

    Some((version, distribution, first_line))
}

/// カッコ内のディストリビューション情報(例: `"TeX Live 2026/Homebrew"`)を抽出する。
fn extract_distribution(text: &str) -> Option<String> {
    for line in text.lines() {
        if let Some(start) = line.find('(')
            && let Some(end) = line[start + 1..].find(')')
        {
            let candidate = &line[start + 1..start + 1 + end];
            if candidate.contains("TeX Live")
                || candidate.contains("MiKTeX")
                || candidate.contains("MacTeX")
            {
                return Some(candidate.trim().to_string());
            }
        }
    }
    None
}

/// コマンド種別に応じたバージョン番号文字列を抽出する。
fn extract_version_number(kind: TexCommandKind, text: &str) -> Option<String> {
    match kind {
        TexCommandKind::Latexmk => {
            // 例: "Latexmk, John Collins, 15 June 2025. Version 4.87"
            extract_word_after(text, "Version")
                .or_else(|| extract_word_after(text, "version"))
                .or_else(|| extract_first_version_pattern(text))
        }
        TexCommandKind::Lualatex => {
            // 例: "This is LuaHBTeX, Version 1.24.0 (TeX Live 2026/Homebrew)"
            extract_word_after(text, "Version")
                .or_else(|| extract_word_after(text, "version"))
                .or_else(|| extract_first_version_pattern(text))
        }
        TexCommandKind::Xelatex => {
            // 例: "XeTeX 3.141592653-2.6-0.999998 (TeX Live 2026/Homebrew)"
            extract_word_after(text, "XeTeX")
                .or_else(|| extract_word_after(text, "Version"))
                .or_else(|| extract_first_version_pattern(text))
        }
        TexCommandKind::Pdflatex => {
            // 例: "pdfTeX 3.141592653-2.6-1.40.29 (TeX Live 2026/Homebrew)"
            extract_word_after(text, "pdfTeX")
                .or_else(|| extract_word_after(text, "Version"))
                .or_else(|| extract_first_version_pattern(text))
        }
    }
}

/// キーワード直後の単語を取り出し、末尾の記号を除去して返す。
fn extract_word_after(text: &str, keyword: &str) -> Option<String> {
    for line in text.lines() {
        if let Some(idx) = line.find(keyword) {
            let after = &line[idx + keyword.len()..];
            let trimmed = after.trim_start_matches(|c: char| c == ':' || c.is_whitespace());
            if let Some(word) = trimmed.split_whitespace().next() {
                let cleaned = word.trim_matches(|c: char| {
                    c == ',' || c == '.' || c == ';' || c == '(' || c == ')'
                });
                if !cleaned.is_empty() {
                    return Some(cleaned.to_string());
                }
            }
        }
    }
    None
}

/// 数字で始まるバージョン番号パターン(例: `"3.14..."`, `"4.87"`)を探索する。
fn extract_first_version_pattern(text: &str) -> Option<String> {
    for word in text.split_whitespace() {
        let cleaned =
            word.trim_matches(|c: char| c == ',' || c == '.' || c == ';' || c == '(' || c == ')');
        if cleaned.starts_with(|c: char| c.is_ascii_digit()) && cleaned.contains('.') {
            return Some(cleaned.to_string());
        }
    }
    None
}

/// `PATH` 環境変数を走査し、指定された実行可能ファイルのパスを探索する。
pub fn find_executable_in_path(command: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(command);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }

        #[cfg(windows)]
        {
            let candidate_exe = dir.join(format!("{command}.exe"));
            if is_executable_file(&candidate_exe) {
                return Some(candidate_exe);
            }
        }
    }
    None
}

/// 指定したパスが存在する実行可能ファイルであるか検査する。
fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = path.metadata() {
            return metadata.permissions().mode() & 0o111 != 0;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_latexmk_version_output() {
        let output = "Latexmk, John Collins, 15 June 2025. Version 4.87\n";
        let (version, dist, first_line) = parse_version_output(TexCommandKind::Latexmk, output, "")
            .expect("`latexmk` のバージョン解析に失敗しました。");

        assert_eq!(version, "4.87");
        assert_eq!(dist, None);
        assert_eq!(
            first_line,
            "Latexmk, John Collins, 15 June 2025. Version 4.87"
        );
    }

    #[test]
    fn test_parse_lualatex_version_output() {
        let output =
            "This is LuaHBTeX, Version 1.24.0 (TeX Live 2026/Homebrew)\nDevelopment id: 7724\n";
        let (version, dist, first_line) =
            parse_version_output(TexCommandKind::Lualatex, output, "")
                .expect("`lualatex` のバージョン解析に失敗しました。");

        assert_eq!(version, "1.24.0");
        assert_eq!(dist, Some("TeX Live 2026/Homebrew".to_string()));
        assert_eq!(
            first_line,
            "This is LuaHBTeX, Version 1.24.0 (TeX Live 2026/Homebrew)"
        );
    }

    #[test]
    fn test_parse_xelatex_version_output() {
        let output =
            "XeTeX 3.141592653-2.6-0.999998 (TeX Live 2026/Homebrew)\nkpathsea version 6.4.2\n";
        let (version, dist, first_line) = parse_version_output(TexCommandKind::Xelatex, output, "")
            .expect("`xelatex` のバージョン解析に失敗しました。");

        assert_eq!(version, "3.141592653-2.6-0.999998");
        assert_eq!(dist, Some("TeX Live 2026/Homebrew".to_string()));
        assert_eq!(
            first_line,
            "XeTeX 3.141592653-2.6-0.999998 (TeX Live 2026/Homebrew)"
        );
    }

    #[test]
    fn test_parse_pdflatex_version_output() {
        let output =
            "pdfTeX 3.141592653-2.6-1.40.29 (TeX Live 2026/Homebrew)\nkpathsea version 6.4.2\n";
        let (version, dist, first_line) =
            parse_version_output(TexCommandKind::Pdflatex, output, "")
                .expect("`pdflatex` のバージョン解析に失敗しました。");

        assert_eq!(version, "3.141592653-2.6-1.40.29");
        assert_eq!(dist, Some("TeX Live 2026/Homebrew".to_string()));
        assert_eq!(
            first_line,
            "pdfTeX 3.141592653-2.6-1.40.29 (TeX Live 2026/Homebrew)"
        );
    }

    #[test]
    fn test_tex_command_kind_properties() {
        assert_eq!(TexCommandKind::Latexmk.as_str(), "latexmk");
        assert_eq!(TexCommandKind::Lualatex.display_name(), "LuaLaTeX");
        assert_eq!(TexCommandKind::all().len(), 4);
        assert_eq!(TexCommandKind::Xelatex.to_string(), "XeLaTeX");
    }

    #[test]
    fn test_tex_environment_helpers() {
        let mut env = TexEnvironment::default();
        assert!(!env.is_available(TexCommandKind::Lualatex));
        assert_eq!(env.preferred_engine(), None);

        env.tools.push(TexToolInfo {
            kind: TexCommandKind::Pdflatex,
            path: Some(PathBuf::from("/usr/bin/pdflatex")),
            version: "1.40.29".to_string(),
            distribution: Some("TeX Live 2026".to_string()),
            raw_version: "pdfTeX 1.40.29".to_string(),
        });

        assert!(env.is_available(TexCommandKind::Pdflatex));
        assert_eq!(env.preferred_engine(), Some(TexCommandKind::Pdflatex));
        assert!(env.pdflatex().is_some());
        assert!(env.lualatex().is_none());

        env.tools.push(TexToolInfo {
            kind: TexCommandKind::Latexmk,
            path: Some(PathBuf::from("/usr/bin/latexmk")),
            version: "4.87".to_string(),
            distribution: None,
            raw_version: "Latexmk 4.87".to_string(),
        });

        assert_eq!(env.preferred_engine(), Some(TexCommandKind::Latexmk));
    }
}
