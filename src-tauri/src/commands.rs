//! フロントエンドと連携するTauriのIPCコマンドモジュール。

use serde::{Deserialize, Serialize};

/// フロントエンドへ送信する画像ビットマップ。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BitmapPayload {
    /// 画像の幅(ピクセル)。
    pub width: u32,
    /// 画像の高さ(ピクセル)。
    pub height: u32,
    /// RGBAバイト列。
    pub rgba_data: Vec<u8>,
}

/// 検証用のテストパターンビットマップを生成する。
#[tauri::command]
pub fn generate_test_bitmap(width: u32, height: u32) -> BitmapPayload {
    let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);

    for y in 0..height {
        for x in 0..width {
            // スライドの外枠を白で描画する。
            if x == 0 || x == width - 1 || y == 0 || y == height - 1 {
                rgba_data.extend_from_slice(&[255, 255, 255, 255]);
                continue;
            }

            // 十字のガイド線を描画する。
            if x == width / 2 || y == height / 2 {
                rgba_data.extend_from_slice(&[200, 200, 220, 255]);
                continue;
            }

            // 背景にグラデーションを描画する。
            let r = ((x as f32 / width as f32) * 180.0 + 30.0) as u8;
            let g = ((y as f32 / height as f32) * 160.0 + 40.0) as u8;
            let b = 220u8;
            let a = 255u8;
            rgba_data.extend_from_slice(&[r, g, b, a]);
        }
    }

    BitmapPayload {
        width,
        height,
        rgba_data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_test_bitmap() {
        let width = 160;
        let height = 90;
        let payload = generate_test_bitmap(width, height);

        assert_eq!(payload.width, width);
        assert_eq!(payload.height, height);
        assert_eq!(payload.rgba_data.len(), (width * height * 4) as usize);
    }
}
