//! BeamerCanvas のコア機能を提供するクレート。
//!
//! プロジェクト管理、設定、キャッシュキーの生成、エラー型の定義を担当する。

pub mod cache;
pub mod error;
pub mod project;

pub use cache::{AssetDependency, ContentHasher, SlideCacheKey};
pub use error::{CoreError, Result};
pub use project::{Project, ProjectConfig};
