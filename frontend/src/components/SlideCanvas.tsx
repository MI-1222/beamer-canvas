import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { RefreshCw, CheckCircle2, AlertCircle } from 'lucide-react';

/**
 * Rustから受信する画像ビットマップのデータ型。
 */
interface BitmapPayload {
  /** 画像の幅(ピクセル)。 */
  width: number;
  /** 画像の高さ(ピクセル)。 */
  height: number;
  /** RGBAバイト列。 */
  rgba_data: number[];
}

/**
 * スライド描画用のキャンバスコンポーネント。
 * TauriのIPC通信でRustから取得したビットマップをCanvasへ描画する。
 */
export function SlideCanvas() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [loading, setLoading] = useState(false);
  const [statusText, setStatusText] = useState<string>('未描画');
  const [errorText, setErrorText] = useState<string | null>(null);
  const [bitmapInfo, setBitmapInfo] = useState<{ width: number; height: number } | null>(null);

  /**
   * RustのTauriコマンドを実行してビットマップを取得し、キャンバスに描画する。
   */
  const handleRenderTestBitmap = async () => {
    setLoading(true);
    setErrorText(null);

    try {
      // 16:9の比率(640x360)でビットマップの生成を要求する。
      const width = 640;
      const height = 360;

      const payload = await invoke<BitmapPayload>('generate_test_bitmap', {
        width,
        height,
      });

      const canvas = canvasRef.current;
      if (!canvas) {
        throw new Error('キャンバス要素が見つかりません。');
      }

      canvas.width = payload.width;
      canvas.height = payload.height;

      const ctx = canvas.getContext('2d');
      if (!ctx) {
        throw new Error('2Dコンテキストの取得に失敗しました。');
      }

      // 受信したRGBAバイト列からImageDataを生成し、キャンバスへ転送する。
      const clampedArray = new Uint8ClampedArray(payload.rgba_data);
      const imageData = new ImageData(clampedArray, payload.width, payload.height);
      ctx.putImageData(imageData, 0, 0);

      setBitmapInfo({ width: payload.width, height: payload.height });
      setStatusText('描画成功(Tauri IPC通信確認済み)');
    } catch (err) {
      console.error('Tauri IPC 呼び出しエラー:', err);
      setErrorText(err instanceof Error ? err.message : String(err));
      setStatusText('描画失敗');
    } finally {
      setLoading(false);
    }
  };

  // 初回マウント時にテスト描画を実行する。
  useEffect(() => {
    void handleRenderTestBitmap();
  }, []);

  return (
    <div className='flex h-full w-full flex-col bg-panel-bg text-zinc-100'>
      <div className='flex items-center justify-between border-b border-border-subtle bg-panel-header px-4 py-2'>
        <div className='flex items-center gap-2'>
          <span className='text-xs font-semibold uppercase tracking-wider text-zinc-400'>
            Slide Preview (Canvas)
          </span>
          {bitmapInfo && (
            <span className='rounded bg-zinc-800 px-2 py-0.5 text-xs text-zinc-300'>
              {bitmapInfo.width} × {bitmapInfo.height} px
            </span>
          )}
        </div>
        <button
          onClick={() => void handleRenderTestBitmap()}
          disabled={loading}
          className='flex items-center gap-1.5 rounded bg-zinc-800 px-3 py-1.5 text-xs font-medium text-zinc-200 transition-colors hover:bg-zinc-700 disabled:opacity-50'
        >
          <RefreshCw className={`h-3.5 w-3.5 ${loading ? 'animate-spin' : ''}`} />
          再描画 (Tauri IPC)
        </button>
      </div>

      <div className='flex flex-1 flex-col items-center justify-center overflow-auto p-6'>
        <div className='relative flex flex-col items-center rounded-lg border border-border-subtle bg-zinc-950/60 p-4 shadow-xl'>
          <canvas
            ref={canvasRef}
            className='max-h-[70vh] max-w-full rounded border border-zinc-800 shadow-lg'
            style={{ aspectRatio: '16 / 9' }}
          />

          <div className='mt-4 flex items-center gap-2 text-xs'>
            {errorText ? (
              <span className='flex items-center gap-1 text-red-400'>
                <AlertCircle className='h-4 w-4' />
                エラー: {errorText}
              </span>
            ) : bitmapInfo ? (
              <span className='flex items-center gap-1 text-emerald-400'>
                <CheckCircle2 className='h-4 w-4' />
                {statusText}
              </span>
            ) : (
              <span className='text-zinc-500'>{statusText}</span>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
