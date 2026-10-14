//! Beamer のスライド構造と PDF 物理ページ・オーバーレイステップのマッピングモジュール。
//!
//! `.nav` 中間ファイルの解析および PDF `/PageLabels` によるフォールバックを提供し、
//! CST 上のフレーム(スライド)と PDF ページの双方向同期を実現する。

pub mod nav_parser;
pub mod table;

pub use nav_parser::{NavFrameEntry, NavParser, NavStepEntry};
pub use table::{FrameMapping, SlideMappingTable};
