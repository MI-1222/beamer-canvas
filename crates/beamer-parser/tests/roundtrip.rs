//! 構文解析の往復完全性(`parse(source).to_string() == source`)と編集操作の可逆性を検証するテスト。

use beamer_parser::parse;

#[test]
fn test_parse_roundtrip_identity() {
    // 1. 空文字、空白のみ、改行のみ。
    for source in ["", "   ", "\n", "\n\n\n", "  \n  \t  \n  "] {
        assert_eq!(parse(source).to_string(), source);
        assert_eq!(parse(source).text(), source);
    }

    // 2. 最小構成の文書。
    let min_doc = "\\documentclass{beamer}\n\\begin{document}\n\\end{document}\n";
    assert_eq!(parse(min_doc).to_string(), min_doc);

    // 3. プリアンブルのみ。
    let preamble_only = "% プリアンブルのみ\n\\documentclass[12pt]{beamer}\n\\usetheme{Madrid}\n";
    assert_eq!(parse(preamble_only).to_string(), preamble_only);

    // 4. `\\end{document}` 以降の後続テキスト。
    let post_doc =
        "\\documentclass{beamer}\n\\begin{document}\n\\end{document}\n% 後続コメント\nテキスト\n";
    assert_eq!(parse(post_doc).to_string(), post_doc);

    // 5. CRLF 改行を含む文書。
    let crlf_doc = "\\documentclass{beamer}\r\n\\begin{document}\r\n\\begin{frame}{タイトル}\r\n  本文\r\n\\end{frame}\r\n\\end{document}\r\n";
    assert_eq!(parse(crlf_doc).to_string(), crlf_doc);

    // 6. 複雑なコメントとインデント。
    let comment_doc = r#"% ファイルヘッダーコメント
\documentclass{beamer} % 行末コメント
% 連続したコメント行1
% 連続したコメント行2
\begin{document}
  % スライド前のインデントコメント
  \begin{frame}{タイトル} % タイトルコメント
    % 本文コメント
    スライド本文 % 行末コメント
  \end{frame}
  % スライド後のコメント
\end{document}
% フッターコメント
"#;
    assert_eq!(parse(comment_doc).to_string(), comment_doc);

    // 7. UTF-8 の日本語・全角記号・特殊文字。
    let ja_doc = r#"\documentclass{beamer}
\usepackage{luatexja}
\title{日本語プレゼンテーション：研究発表}
\author{山田 太郎}
\begin{document}
\begin{frame}{日本語タイトル}{サブタイトル：概要と目的}
  \begin{itemize}
    \item 項目1：人工知能と自然言語処理。
    \item 項目2：数式 $E = mc^2$ と記号(※、★、♪)。
  \end{itemize}
\end{frame}
\end{document}
"#;
    assert_eq!(parse(ja_doc).to_string(), ja_doc);

    // 8. 旧式マクロ `\frame{...}`。
    let legacy_doc = "\\documentclass{beamer}\n\\begin{document}\n\\frame{ 旧式スライド本文 }\n\\end{document}\n";
    assert_eq!(parse(legacy_doc).to_string(), legacy_doc);
}

#[test]
fn test_parse_roundtrip_error_recovery() {
    // 構文エラーや閉じタグの欠落があっても、元のソースコードを1バイトも欠落させずに保持する。

    // 1. 未閉じの `frame` 環境。
    let unclosed_frame = r#"\documentclass{beamer}
\begin{document}
\begin{frame}{未完了スライド}
  本文テキスト
\end{document}
"#;
    let res1 = parse(unclosed_frame);
    assert!(res1.has_errors());
    assert_eq!(res1.to_string(), unclosed_frame);

    // 2. 未閉じの `document` 環境。
    let unclosed_doc = r#"\documentclass{beamer}
\begin{document}
\begin{frame}{スライド}
  本文
\end{frame}
"#;
    let res2 = parse(unclosed_doc);
    assert!(res2.has_errors());
    assert_eq!(res2.to_string(), unclosed_doc);

    // 3. 閉じ括弧が欠落したマクロ引数。
    let unclosed_arg = r#"\documentclass{beamer}
\begin{document}
\begin{frame}{未完了タイトル
  本文
\end{frame}
\end{document}
"#;
    let res3 = parse(unclosed_arg);
    assert_eq!(res3.to_string(), unclosed_arg);

    // 4. 壊れた環境名と孤立したバックスラッシュ。
    let broken_env = r#"\documentclass{beamer}
\begin{document}
\begin{
\
\begin{frame}{正常スライド}
  本文
\end{frame}
\end{document}
"#;
    let res4 = parse(broken_env);
    assert_eq!(res4.to_string(), broken_env);
}

#[test]
fn test_edit_identity_roundtrip() {
    // `DocumentEditor` を介して何も編集を行わない場合、元のソースコードと完全一致する。
    let source = r#"\documentclass{beamer}
\begin{document}

\begin{frame}{スライド1}
  内容1
\end{frame}

\section{セクション名}

\begin{frame}{スライド2}
  内容2
\end{frame}

\end{document}
"#;

    let res = parse(source);
    let editor = res.editor();
    assert_eq!(editor.to_string(), source);
    assert_eq!(editor.text(), source);
}

#[test]
fn test_edit_reversibility_swap_frames() {
    // スライドAとBを入れ替え、再度入れ替えて元の順序に戻したとき、元のソースコードと完全に一致する。
    let source = r#"\documentclass{beamer}
\begin{document}

\begin{frame}{スライドA}
  スライドAの内容
\end{frame}

\begin{frame}{スライドB}
  スライドBの内容
\end{frame}

\end{document}
"#;

    let res = parse(source);
    let editor = res.editor();

    // スライド 0 と 1 を入れ替える。
    let editor_swapped = editor
        .swap_frames(0, 1)
        .expect("スライドの入れ替えに失敗。");
    let swapped = editor_swapped.to_string();
    assert_ne!(swapped, source);

    // 再度スライド 0 と 1 を入れ替えて元の順序に戻す。
    let editor_restore = editor_swapped
        .swap_frames(0, 1)
        .expect("スライドの再入れ替えに失敗。");

    assert_eq!(editor_restore.to_string(), source);
}

#[test]
fn test_edit_reversibility_swap_with_interleaved_section() {
    // スライドの間にセクションやコメントが挟まれていても、スライドを2回入れ替えると元の状態に戻る。
    let source = r#"\documentclass{beamer}
\begin{document}

\begin{frame}{スライド1}
  スライド1の内容
\end{frame}

% ここから第2セクション
\section{詳細設計}

\begin{frame}{スライド2}
  スライド2の内容
\end{frame}

\end{document}
"#;

    let res = parse(source);
    let editor = res.editor();

    // スライド 0 と 1 を入れ替える。
    let editor_swapped = editor
        .swap_frames(0, 1)
        .expect("スライドの入れ替えに失敗。");
    let swapped = editor_swapped.to_string();

    // セクション見出しとコメントが元の位置に保持されていることを確認する。
    assert!(swapped.contains("% ここから第2セクション\n\\section{詳細設計}"));

    // 再度入れ替えて元に戻す。
    let editor_restore = editor_swapped
        .swap_frames(0, 1)
        .expect("スライドの再入れ替えに失敗。");

    assert_eq!(editor_restore.to_string(), source);
}

#[test]
fn test_edit_reversibility_move_frame() {
    // スライドを移動し、逆方向に再度移動して元の位置に戻したとき、元のソースコードと完全に一致する。
    let source = r#"\documentclass{beamer}
\begin{document}

\begin{frame}{第1スライド}
  第1スライド本文
\end{frame}

\begin{frame}{第2スライド}
  第2スライド本文
\end{frame}

\begin{frame}{第3スライド}
  第3スライド本文
\end{frame}

\end{document}
"#;

    // 先頭スライド (0) を末尾 (2) へ移動し、再度先頭へ戻す。
    let res = parse(source);
    let editor = res.editor();
    let moved_to_end = editor.move_frame(0, 2).expect("スライドの末尾移動に失敗。");

    let editor_restore = moved_to_end
        .move_frame(2, 0)
        .expect("スライドの先頭復帰移動に失敗。");

    assert_eq!(editor_restore.to_string(), source);

    // 末尾スライド (2) を先頭 (0) へ移動し、再度末尾へ戻す。
    let editor2 = parse(source).editor();
    let moved_to_front = editor2
        .move_frame(2, 0)
        .expect("スライドの先頭移動に失敗。");

    let editor2_restore = moved_to_front
        .move_frame(0, 2)
        .expect("スライドの末尾復帰移動に失敗。");

    assert_eq!(editor2_restore.to_string(), source);
}

#[test]
fn test_edit_reversibility_insert_and_delete_frame() {
    // スライドを挿入した直後に削除したとき、余分な空行やインデントの乱れがなく元のコードと一致する。
    let source = r#"\documentclass{beamer}
\begin{document}

\begin{frame}{初期スライド1}
  初期スライド1の内容
\end{frame}

\begin{frame}{初期スライド2}
  初期スライド2の内容
\end{frame}

\end{document}
"#;

    // 1. 中間に挿入して削除。
    let res = parse(source);
    let editor = res.editor();
    let template = "\\begin{frame}{新規挿入スライド}\n  新規内容\n\\end{frame}\n";
    let inserted = editor
        .insert_frame(1, template)
        .expect("スライドの中間挿入に失敗。");
    assert_eq!(inserted.frame_count().unwrap(), 3);

    let editor_del = inserted
        .delete_frame(1)
        .expect("挿入したスライドの削除に失敗。");
    assert_eq!(editor_del.to_string(), source);

    // 2. 先頭に挿入して削除。
    let editor_head = parse(source).editor();
    let inserted_head = editor_head
        .insert_frame(0, template)
        .expect("スライドの先頭挿入に失敗。");

    let editor_head_del = inserted_head
        .delete_frame(0)
        .expect("先頭スライドの削除に失敗。");
    assert_eq!(editor_head_del.to_string(), source);

    // 3. 末尾に挿入して削除。
    let editor_tail = parse(source).editor();
    let inserted_tail = editor_tail
        .insert_frame(2, template)
        .expect("スライドの末尾挿入に失敗。");

    let editor_tail_del = inserted_tail
        .delete_frame(2)
        .expect("末尾スライドの削除に失敗。");
    assert_eq!(editor_tail_del.to_string(), source);
}

#[test]
fn test_edit_reversibility_property_changes() {
    // プロパティを変更した後に元の値へ戻したとき、元のソースコードと完全に一致する。
    let source = r#"\documentclass{beamer}
\begin{document}

\begin{frame}[plain]{元のタイトル}{元のサブタイトル}
  \includegraphics[width=0.5\textwidth]{images/figure.png}
\end{frame}

\end{document}
"#;

    let res = parse(source);
    let editor = res.editor();

    // 1. タイトル変更して元に戻す。
    let edited_title = editor
        .set_frame_title(0, "新しいタイトル")
        .expect("タイトルの更新に失敗。");
    let editor_restore_title = edited_title
        .set_frame_title(0, "元のタイトル")
        .expect("タイトルの復元に失敗。");
    assert_eq!(editor_restore_title.to_string(), source);

    // 2. サブタイトル変更して元に戻す。
    let edited_sub = editor
        .set_frame_subtitle(0, "新しいサブタイトル")
        .expect("サブタイトルの更新に失敗。");
    let editor_restore_sub = edited_sub
        .set_frame_subtitle(0, "元のサブタイトル")
        .expect("サブタイトルの復元に失敗。");
    assert_eq!(editor_restore_sub.to_string(), source);

    // 3. オプション変更して元に戻す。
    let edited_opt = editor
        .set_frame_options(0, "shrink=10")
        .expect("オプションの更新に失敗。");
    let editor_restore_opt = edited_opt
        .set_frame_options(0, "plain")
        .expect("オプションの復元に失敗。");
    assert_eq!(editor_restore_opt.to_string(), source);

    // 4. 画像幅変更して元に戻す。
    let edited_img = editor
        .set_image_width(0, 0, "0.8\\linewidth")
        .expect("画像幅の更新に失敗。");
    let editor_restore_img = edited_img
        .set_image_width(0, 0, "0.5\\textwidth")
        .expect("画像幅の復元に失敗。");
    assert_eq!(editor_restore_img.to_string(), source);
}

#[test]
fn test_roundtrip_complex_beamer_document() {
    // 多様な構文を含む実際の `Beamer` プレゼンテーションを用いた往復完全性の検証テスト。
    let source = r#"\documentclass[aspectratio=169,10pt]{beamer}

% テーマとカラー設定
\usetheme{Madrid}
\usecolortheme{whale}

% パッケージ宣言
\usepackage{luatexja}
\usepackage{amsmath,amssymb}
\usepackage{graphicx}
\usepackage{tikz}

% プレゼンテーションメタデータ
\title[短縮タイトル]{完全なラウンドトリップを実証するBeamerプレゼンテーション}
\subtitle{ロスレスアイランドパースの検証}
\author[発表者略称]{山田 太郎 \inst{1} \and 鈴木 次郎 \inst{2}}
\institute[機関略称]{\inst{1} 東京研究所 \and \inst{2} 京都開発部}
\date[\today]{\today}

\begin{document}

% タイトルページ
\begin{frame}[plain]
  \titlepage
\end{frame}

% 目次
\begin{frame}{全体の目次}
  \tableofcontents
\end{frame}

\section{基本概念と導入}

\begin{frame}<1-3>{第1節：導入と動機}{オーバーレイとブロックの共存}
  \begin{itemize}[<+->]
    \item 最初の論点：CSTパーサーの不変性とロスレス性。
    \item 次の論点：TeXの任意構文に対する高いエラー耐性。
    \item 最後の論点：GUI双方向同期の実現可能性。
  \end{itemize}

  \pause

  \begin{alertblock}{重要な警告}
    コメントやインデントを1バイトも欠落させてはならない。
  \end{alertblock}
\end{frame}

\section{詳細設計と実装}

\begin{frame}{第2節：2段組と作図環境}
  \begin{columns}[T]
    \begin{column}{0.48\textwidth}
      \begin{block}{数式モデル}
        シュレーディンガー方程式：
        \begin{equation}
          i\hbar \frac{\partial}{\partial t}\Psi = \hat{H}\Psi
        \end{equation}
      \end{block}
    \end{column}

    \begin{column}{0.48\textwidth}
      \begin{exampleblock}{TikZ 作図ブロック}
        % TikZ による作図(不透明ブロックとして保持される)
        \begin{tikzpicture}
          \draw[thick,->] (0,0) -- (3,0) node[anchor=north west] {$x$};
          \draw[thick,->] (0,0) -- (0,2) node[anchor=south_east] {$y$};
          \draw[red] (0,0) rectangle (2,1.5);
        \end{tikzpicture}
      \end{exampleblock}
    \end{column}
  \end{columns}
\end{frame}

\begin{frame}{第3節：画像の配置と余白調整}
  \centering
  \vspace{5pt}
  \includegraphics[width=0.6\linewidth,keepaspectratio]{assets/diagram.png}
  \vspace{10pt}

  \begin{enumerate}
    \item ステップ1：パーサーによる木構造構築。
    \item ステップ2：GUIによるノードプロパティ編集。
  \end{enumerate}
\end{frame}

\section{まとめと今後の展望}

\begin{frame}{最終スライド}
  \frametitle{まとめ}
  \begin{itemize}
    \item すべてのトークンが完全に保存された。
    \item `to_string()` による出力が入力ソースと100\%一致する。
  \end{itemize}
\end{frame}

\end{document}
% ドキュメント末尾のコメント
"#;

    // 1. 解析結果が元のソースコードと1バイトの狂いもなく一致することを確認する。
    let result = parse(source);
    assert!(!result.has_errors());
    assert_eq!(result.to_string(), source);
    assert_eq!(result.text(), source);

    // 2. `DocumentEditor` を介しても完全に一致することを確認する。
    let editor = result.editor();
    assert_eq!(editor.to_string(), source);

    // 3. スライドの抽出数が正しいことを確認する。
    let frames = editor.get_frames().expect("スライドの抽出に失敗。");
    assert_eq!(frames.len(), 6);

    // 4. スライド2と3を入れ替えてから元に戻し、可逆性を確認する。
    let editor_swapped = result
        .editor()
        .swap_frames(2, 3)
        .expect("スライドの入れ替えに失敗。");

    let editor_restored = editor_swapped
        .swap_frames(2, 3)
        .expect("スライドの再入れ替えに失敗。");
    assert_eq!(editor_restored.to_string(), source);
}
