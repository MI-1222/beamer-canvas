//! `CST`編集およびアウトライン抽出に関するエラー型モジュール。

use std::fmt;

/// `CST`編集や要素抽出処理で発生するエラーを表す列挙型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// ドキュメント本文 `\begin{document}` が存在しない。
    DocumentBodyNotFound,

    /// 構文エラーノード `ErrorNode` またはパースエラーが存在するため安全に処理できない。
    SyntaxError {
        /// エラーの詳細メッセージ。
        message: String,
        /// エラーが発生したスライド等のインデックス(特定可能な場合)。
        index: Option<usize>,
    },

    /// 指定されたインデックスが範囲外である。
    IndexOutOfBounds {
        /// 指定されたインデックス。
        index: usize,
        /// 有効な要素数。
        max: usize,
    },

    /// 対象の要素（画像やタイトルなど）が見つからない。
    ElementNotFound {
        /// エラーの詳細メッセージ。
        message: String,
    },
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DocumentBodyNotFound => write!(f, "`document` 環境が見つからない。"),
            Self::SyntaxError { message, index } => {
                if let Some(idx) = index {
                    write!(
                        f,
                        "インデックス {} に構文エラーが存在する: {}",
                        idx, message
                    )
                } else {
                    write!(f, "構文エラーが存在する: {}", message)
                }
            }
            Self::IndexOutOfBounds { index, max } => {
                write!(
                    f,
                    "インデックス {} は範囲外である(有効要素数: {})。",
                    index, max
                )
            }
            Self::ElementNotFound { message } => {
                write!(f, "対象要素が見つからない: {}", message)
            }
        }
    }
}

impl std::error::Error for EditError {}
