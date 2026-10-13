//! キャッシュキー生成およびコンテンツハッシュ管理モジュール。

/// スライドコードや設定からコンテンツハッシュ（BLAKE3）を算出して一意のキーを生成する構造体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentHasher;

impl ContentHasher {
    /// 与えられたテキストコンテンツから BLAKE3 ハッシュ文字列（16進数表記）を生成する。
    pub fn compute_hash(content: &str) -> String {
        let hash = blake3::hash(content.as_bytes());
        hash.to_hex().to_string()
    }

    /// スライドのソースコードとプリアンブルを連結したハッシュキーを生成する。
    pub fn compute_slide_key(preamble: &str, slide_source: &str) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(preamble.as_bytes());
        hasher.update(b"\n---slide_separator---\n");
        hasher.update(slide_source.as_bytes());
        hasher.finalize().to_hex().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash() {
        let hash_a = ContentHasher::compute_hash("hello");
        let hash_b = ContentHasher::compute_hash("hello");
        let hash_c = ContentHasher::compute_hash("world");

        assert_eq!(hash_a, hash_b);
        assert_ne!(hash_a, hash_c);
    }
}
