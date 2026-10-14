//! `TeX` 環境自動検出機能の統合テスト。

use beamer_renderer::{TexCommandKind, TexEnvironment};

#[tokio::test]
async fn test_tex_environment_detect_real_system() {
    let env = TexEnvironment::detect().await;

    // ローカル環境に導入されている `TeX` コマンドを検証する。
    // `macOS` 環境などで `TeX Live` が導入されている場合、各種コマンドを検出する。
    println!("検出された `TeX` ツール数: {}", env.tools.len());
    for tool in &env.tools {
        println!(
            "ツール: {:?}, パス: {:?}, バージョン: {}, ディストリビューション: {:?}, 生出力: {}",
            tool.kind, tool.path, tool.version, tool.distribution, tool.raw_version
        );
        assert!(
            !tool.version.is_empty(),
            "バージョン文字列が空のため検証に失敗。"
        );
        assert!(
            !tool.raw_version.is_empty(),
            "生バージョン情報の取得に失敗。"
        );
        if let Some(ref path) = tool.path {
            assert!(
                path.exists(),
                "検出された実行可能ファイルが存在しないため検証に失敗。"
            );
        }
    }

    // `latexmk` または `lualatex` が利用可能な場合、推奨エンジンの選定結果を検証する。
    if env.is_available(TexCommandKind::Latexmk) {
        assert_eq!(
            env.preferred_engine(),
            Some(TexCommandKind::Latexmk),
            "`latexmk` の優先選定に失敗。"
        );
        let latexmk_info = env.latexmk().expect("`latexmk` 情報の取得に失敗しました。");
        assert_eq!(latexmk_info.kind, TexCommandKind::Latexmk);
    }

    if env.is_available(TexCommandKind::Lualatex) {
        let lualatex_info = env
            .lualatex()
            .expect("`lualatex` 情報の取得に失敗しました。");
        assert_eq!(lualatex_info.kind, TexCommandKind::Lualatex);
    }
}

#[tokio::test]
async fn test_tex_environment_serde_roundtrip() {
    let env = TexEnvironment::detect().await;

    let json = serde_json::to_string(&env).expect("`JSON` シリアライズに失敗しました。");
    let deserialized: TexEnvironment =
        serde_json::from_str(&json).expect("`JSON` デシリアライズに失敗しました。");

    assert_eq!(
        env, deserialized,
        "シリアライズ前後の不一致が原因で失敗しました。"
    );
}
