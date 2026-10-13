//! パーサー(`BeamerParser`)モジュールの単体テスト。

use beamer_parser::BeamerParser;

#[test]
fn test_parse_roundtrip() {
    let source = "\\documentclass{beamer}\n\\begin{document}\n\\end{document}";
    let parser = BeamerParser::new();
    let result = parser.parse(source);

    assert_eq!(result.syntax.text().to_string(), source);
}
