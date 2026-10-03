import type { MouseEvent } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import styles from './Markdown.module.css';

/**
 * Renders model output as Markdown (GitHub flavour: tables, task lists, strikethrough).
 *
 * Raw HTML in the text is never rendered, and links do not navigate: a click inside the
 * app's webview would replace Atlas itself, and Atlas has no permission to open a browser.
 */
export function Markdown({ children }: { children: string }) {
  return (
    <div className={styles.markdown}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          a: ({ href, children: label }) => (
            <a
              href={href}
              title={href}
              onClick={(event: MouseEvent) => {
                event.preventDefault();
              }}
            >
              {label}
            </a>
          ),
        }}
      >
        {children}
      </ReactMarkdown>
    </div>
  );
}
