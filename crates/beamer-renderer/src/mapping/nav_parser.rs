//! Beamer の `.nav`(ナビゲーション中間ファイル)の決定論的パーサー。
//!
//! `\beamer@framepages` および `\slideentry` マクロを解析し、
//! 各フレームの開始・終了ページおよびオーバーレイステップと物理ページの対応関係を抽出する。

/// .nav ファイルから抽出された 1 つのオーバーレイステップエントリ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavStepEntry {
    /// セクション番号(1-based)。
    pub section: usize,
    /// サブセクション番号(1-based)。
    pub subsection: usize,
    /// フレーム内ステップ番号(1-based)。
    pub step: usize,
    /// 物理ページ番号(1-based)。
    pub physical_page: usize,
    /// フレーム内最終物理ページ番号(1-based)。
    pub total_pages_in_frame: usize,
}

/// .nav ファイルから抽出された 1 つのフレームエントリ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavFrameEntry {
    /// 開始物理ページ番号(1-based)。
    pub start_page: usize,
    /// 終了物理ページ番号(1-based)。
    pub end_page: usize,
    /// フレーム内の各ステップエントリ一覧。
    pub steps: Vec<NavStepEntry>,
}

/// `.nav` ファイルの解析器。
pub struct NavParser;

impl NavParser {
    /// `.nav` ファイルのテキストを行ごとに走査し、フレームエントリの一覧を抽出する。
    pub fn parse(nav_content: &str) -> Vec<NavFrameEntry> {
        let mut frames = Vec::new();
        let mut current_steps = Vec::new();

        for line in nav_content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('%') {
                continue;
            }

            // \slideentry の検出。
            if let Some(pos) = trimmed.find("\\slideentry") {
                let rest = &trimmed[pos + "\\slideentry".len()..];
                let args = Self::extract_braced_arguments(rest);
                if args.len() >= 4 {
                    let section = args[0].trim().parse::<usize>().unwrap_or(1);
                    let subsection = args[1].trim().parse::<usize>().unwrap_or(0);
                    let step = args[2].trim().parse::<usize>().unwrap_or(1);

                    // args[3] は "curr/total" 形式(例: "2/4")または単一数値。
                    let (physical_page, total_pages) = Self::parse_page_ratio(args[3].trim());

                    current_steps.push(NavStepEntry {
                        section,
                        subsection,
                        step,
                        physical_page,
                        total_pages_in_frame: total_pages,
                    });
                }
            }

            // \beamer@framepages の検出。
            if let Some(pos) = trimmed.find("\\beamer@framepages") {
                let rest = &trimmed[pos + "\\beamer@framepages".len()..];
                let args = Self::extract_braced_arguments(rest);
                if args.len() >= 2 {
                    let start_page = args[0].trim().parse::<usize>().unwrap_or(1);
                    let end_page = args[1].trim().parse::<usize>().unwrap_or(start_page);

                    let steps = std::mem::take(&mut current_steps);
                    frames.push(NavFrameEntry {
                        start_page,
                        end_page,
                        steps,
                    });
                }
            }
        }

        // \beamer@framepages なしで \slideentry のみが残っていた場合のフォールバック。
        if !current_steps.is_empty() {
            let start_page = current_steps
                .iter()
                .map(|s| s.physical_page)
                .min()
                .unwrap_or(1);
            let end_page = current_steps
                .iter()
                .map(|s| s.physical_page)
                .max()
                .unwrap_or(start_page);

            frames.push(NavFrameEntry {
                start_page,
                end_page,
                steps: current_steps,
            });
        }

        frames
    }

    /// "current/total" 形式(例: "2/4")のページ指定文字列をパースする。
    fn parse_page_ratio(s: &str) -> (usize, usize) {
        if let Some((curr_str, total_str)) = s.split_once('/') {
            let curr = curr_str.trim().parse::<usize>().unwrap_or(1);
            let total = total_str.trim().parse::<usize>().unwrap_or(curr);
            (curr, total)
        } else {
            let val = s.trim().parse::<usize>().unwrap_or(1);
            (val, val)
        }
    }

    /// 文字列中から波括弧 `{...}` で囲まれた引数を順次抽出する。
    ///
    /// ネストされた波括弧やエスケープされた波括弧を考慮して境界を判定する。
    pub fn extract_braced_arguments(input: &str) -> Vec<String> {
        let mut results = Vec::new();
        let bytes = input.as_bytes();
        let len = bytes.len();
        let mut idx = 0;

        while idx < len {
            // 次の開き中括弧 '{' を探索。
            while idx < len && bytes[idx] != b'{' {
                idx += 1;
            }

            if idx >= len {
                break;
            }

            // 開き中括弧 '{' の直後から探索開始。
            idx += 1;
            let start = idx;
            let mut depth = 1;

            while idx < len && depth > 0 {
                // エスケープ文字('\')のチェック。
                if bytes[idx] == b'\\' && idx + 1 < len {
                    idx += 2;
                    continue;
                }

                if bytes[idx] == b'{' {
                    depth += 1;
                } else if bytes[idx] == b'}' {
                    depth -= 1;
                    if depth == 0 {
                        let content = &input[start..idx];
                        results.push(content.to_string());
                        idx += 1;
                        break;
                    }
                }
                idx += 1;
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_braced_arguments() {
        let input = " {1}{0}{2}{2/4}{Title with {nested} braces}{0}";
        let args = NavParser::extract_braced_arguments(input);
        assert_eq!(args.len(), 6);
        assert_eq!(args[0], "1");
        assert_eq!(args[1], "0");
        assert_eq!(args[2], "2");
        assert_eq!(args[3], "2/4");
        assert_eq!(args[4], "Title with {nested} braces");
        assert_eq!(args[5], "0");
    }

    #[test]
    fn test_parse_sample_nav() {
        let nav = r#"
\headcommand {\slideentry {1}{0}{1}{1/1}{}{0}}
\headcommand {\beamer@framepages {1}{1}}
\headcommand {\sectionentry {1}{Introduction}{1}{Introduction}{0}}
\headcommand {\slideentry {1}{0}{2}{2/4}{}{0}}
\headcommand {\slideentry {1}{0}{3}{3/4}{}{0}}
\headcommand {\slideentry {1}{0}{4}{4/4}{}{0}}
\headcommand {\beamer@framepages {2}{4}}
"#;
        let frames = NavParser::parse(nav);
        assert_eq!(frames.len(), 2);

        // Frame 0: 1ページのみ
        assert_eq!(frames[0].start_page, 1);
        assert_eq!(frames[0].end_page, 1);
        assert_eq!(frames[0].steps.len(), 1);
        assert_eq!(frames[0].steps[0].step, 1);
        assert_eq!(frames[0].steps[0].physical_page, 1);

        // Frame 1: 3ステップ(2〜4ページ)
        assert_eq!(frames[1].start_page, 2);
        assert_eq!(frames[1].end_page, 4);
        assert_eq!(frames[1].steps.len(), 3);
        assert_eq!(frames[1].steps[0].step, 2);
        assert_eq!(frames[1].steps[0].physical_page, 2);
        assert_eq!(frames[1].steps[1].step, 3);
        assert_eq!(frames[1].steps[1].physical_page, 3);
        assert_eq!(frames[1].steps[2].step, 4);
        assert_eq!(frames[1].steps[2].physical_page, 4);
    }
}
