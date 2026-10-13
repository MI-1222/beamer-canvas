import { defaultKeymap, history, historyKeymap } from '@codemirror/commands'
import { StreamLanguage } from '@codemirror/language'
import { stex } from '@codemirror/legacy-modes/mode/stex'
import { EditorState } from '@codemirror/state'
import { oneDark } from '@codemirror/theme-one-dark'
import { EditorView, keymap, lineNumbers } from '@codemirror/view'
import { useEffect, useRef } from 'react'

/**
 * CodeEditor コンポーネントのプロパティ定義。
 */
export interface CodeEditorProps {
  /** エディタの初期テキストまたはバインドするテキスト値。 */
  value: string
  /** テキスト変更時のコールバック関数。 */
  onChange?: (value: string) => void
  /** コンテナ要素に付与する追加の CSS クラス名。 */
  className?: string
}

/**
 * CodeMirror 6 を用いた LaTeX/Beamer ソースコード編集用 React コンポーネント。
 */
export function CodeEditor({ value, onChange, className }: CodeEditorProps) {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const viewRef = useRef<EditorView | null>(null)
  const onChangeRef = useRef(onChange)

  useEffect(() => {
    onChangeRef.current = onChange
  }, [onChange])

  useEffect(() => {
    if (!containerRef.current) {
      return
    }

    // LaTeX (stex モード) 用の言語拡張機能。
    const latexLanguage = StreamLanguage.define(stex)

    // ドキュメント更新リスナー拡張。
    const updateListener = EditorView.updateListener.of((update) => {
      if (update.docChanged && onChangeRef.current) {
        onChangeRef.current(update.state.doc.toString())
      }
    })

    // 基本エディタ設定。
    const state = EditorState.create({
      doc: value,
      extensions: [
        lineNumbers(),
        history(),
        keymap.of([...defaultKeymap, ...historyKeymap]),
        latexLanguage,
        oneDark,
        updateListener,
        EditorView.theme({
          '&': {
            height: '100%',
            fontSize: '14px',
            fontFamily: 'var(--font-mono)',
          },
          '.cm-scroller': {
            overflow: 'auto',
          },
        }),
      ],
    })

    const view = new EditorView({
      state,
      parent: containerRef.current,
    })

    viewRef.current = view

    return () => {
      view.destroy()
      viewRef.current = null
    }
    // 初回マウント時のみエディタインスタンスを初期化する。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // 外部からの value 変更を CodeMirror に反映する。
  useEffect(() => {
    const view = viewRef.current
    if (!view) {
      return
    }

    const currentValue = view.state.doc.toString()
    if (value !== currentValue) {
      view.dispatch({
        changes: {
          from: 0,
          to: currentValue.length,
          insert: value,
        },
      })
    }
  }, [value])

  return (
    <div
      ref={containerRef}
      className={`h-full w-full overflow-hidden ${className ?? ''}`}
    />
  )
}
