//! レンダラーモジュールにおけるエラー定義。

use thiserror::Error;

/// レンダリングまたは `TeX` ビルド処理中に発生し得るエラー型。
#[derive(Debug, Error)]
pub enum RenderError {
    /// `PDFium` ライブラリの初期化または操作失敗。
    #[error("PDFium操作に失敗しました: {message}")]
    Pdfium {
        /// エラーの詳細メッセージ。
        message: String,
    },

    /// `TeX` コマンドプロセスの起動・実行エラー。
    #[error("TeXコマンド実行エラー: {0}")]
    Process(#[from] std::io::Error),

    /// `TeX` コンパイルの終了コード異常(ビルド失敗)。
    #[error("TeXコンパイルが異常終了しました (終了コード: {exit_code:?}): {stderr}")]
    CompileFailed {
        /// プロセスの終了コード。
        exit_code: Option<i32>,
        /// 標準出力ログ内容。
        stdout: String,
        /// 標準エラー出力またはログ内容。
        stderr: String,
    },

    /// ファイルが存在しない、または無効なパス指定。
    #[error("無効なファイルパスです: {path}")]
    InvalidPath {
        /// 対象のファイルパス文字列。
        path: String,
    },

    /// ビルド処理のキャンセルまたは中断。
    #[error("ビルド処理がキャンセルされました。")]
    Cancelled,

    /// サンドボックス環境の構築または操作のエラー。
    #[error("サンドボックス操作エラー: {message}")]
    Sandbox {
        /// 詳細メッセージ。
        message: String,
    },

    /// 指定された `TeX` コマンドが環境に見つからない。
    #[error("TeXコマンドが見つかりません: {command}")]
    ToolNotFound {
        /// コマンド名。
        command: String,
    },

    /// キャッシュのエンコード、デコード、またはファイル操作のエラー。
    #[error("キャッシュ操作が原因で失敗しました: {message}")]
    Cache {
        /// 詳細メッセージ。
        message: String,
    },

    /// ページマッピングおよび中間ファイル解析のエラー。
    #[error("ページマッピング処理に失敗しました: {message}")]
    Mapping {
        /// 詳細メッセージ。
        message: String,
    },
}

/// レンダラーモジュールの汎用 `Result` 型エイリアス。
pub type Result<T> = std::result::Result<T, RenderError>;
