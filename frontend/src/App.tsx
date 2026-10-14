import { useState } from 'react';
import { CodeEditor } from './components/CodeEditor';
import { SlideCanvas } from './components/SlideCanvas';

const INITIAL_LATEX_CODE = `\\documentclass{beamer}
\\usetheme{Madrid}

\\title{Sample Presentation}
\\author{Author Name}
\\date{\\today}

\\begin{document}

\\begin{frame}
  \\titlepage
\\end{frame}

\\begin{frame}{Introduction}
  \\begin{itemize}
    \\item First item
    \\item Second item
  \\end{itemize}
\\end{frame}

\\end{document}
`;

/**
 * アプリケーションのメインコンポーネント。
 * コードエディタとスライドプレビューを左右に分割表示する。
 */
export function App() {
  const [code, setCode] = useState(INITIAL_LATEX_CODE);

  return (
    <div className='flex h-screen w-screen flex-col bg-panel-bg text-zinc-100'>
      <header className='flex h-12 shrink-0 items-center border-b border-border-subtle bg-panel-header px-4'>
        <h1 className='text-sm font-semibold tracking-wide'>
          BeamerCanvas - Setup Verification
        </h1>
      </header>
      <main className='flex flex-1 overflow-hidden'>
        {/* 左ペイン: LaTeXコードエディタ */}
        <div className='flex flex-1 flex-col border-r border-border-subtle overflow-hidden'>
          <div className='flex items-center border-b border-border-subtle bg-panel-header px-4 py-2 text-xs font-semibold uppercase tracking-wider text-zinc-400'>
            Code Editor
          </div>
          <div className='flex-1 overflow-hidden p-2'>
            <div className='h-full w-full overflow-hidden rounded border border-border-subtle'>
              <CodeEditor value={code} onChange={setCode} />
            </div>
          </div>
        </div>

        {/* 右ペイン: スライドキャンバス描画 */}
        <div className='flex flex-1 flex-col overflow-hidden'>
          <SlideCanvas />
        </div>
      </main>
    </div>
  );
}

export default App;
