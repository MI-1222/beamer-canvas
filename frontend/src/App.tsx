import { useState } from 'react';
import { CodeEditor } from './components/CodeEditor';

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
 */
export function App() {
  const [code, setCode] = useState(INITIAL_LATEX_CODE);

  return (
    <div className='flex h-screen w-screen flex-col bg-panel-bg text-zinc-100'>
      <header className='flex h-12 items-center border-b border-border-subtle bg-panel-header px-4'>
        <h1 className='text-base font-semibold tracking-wide'>
          Beamer Editor - Setup Verification
        </h1>
      </header>
      <main className='flex-1 overflow-hidden p-4'>
        <div className='h-full w-full overflow-hidden rounded-md border border-border-subtle'>
          <CodeEditor value={code} onChange={setCode} />
        </div>
      </main>
    </div>
  );
}

export default App;
