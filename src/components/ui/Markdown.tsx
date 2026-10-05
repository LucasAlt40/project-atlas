import { Children, isValidElement, useState, type MouseEvent, type ReactNode } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { useT } from '@/i18n/I18nProvider';
import styles from './Markdown.module.css';

function textOf(node: ReactNode): string {
  if (typeof node === 'string') return node;
  if (Array.isArray(node)) return node.map(textOf).join('');
  if (isValidElement<{ children?: ReactNode }>(node)) return textOf(node.props.children);
  return '';
}

const JSON_TOKEN =
  /("(?:\\.|[^"\\])*")(\s*:)?|\b(true|false|null)\b|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)/g;

/** JSON as lines of colored tokens: keys, strings, numbers and literals each get a class. */
function highlightJson(source: string): ReactNode[] {
  const out: ReactNode[] = [];
  let last = 0;
  for (const match of source.matchAll(JSON_TOKEN)) {
    const at = match.index;
    if (at > last) out.push(source.slice(last, at));
    const [token, str, colon, literal] = match;
    if (str !== undefined) {
      out.push(
        <span key={at} className={colon ? styles.jsonKey : styles.jsonString}>
          {str}
        </span>,
      );
      if (colon) out.push(colon);
    } else {
      out.push(
        <span key={at} className={literal ? styles.jsonLiteral : styles.jsonNumber}>
          {token}
        </span>,
      );
    }
    last = at + token.length;
  }
  if (last < source.length) out.push(source.slice(last));
  return out;
}

/** The pretty-printed form when the block is JSON (whatever it declares), else null. */
function prettyJson(language: string, source: string): string | null {
  const text = source.trim();
  if (language !== 'json' && !/^[[{]/.test(text)) return null;
  try {
    return JSON.stringify(JSON.parse(text), null, 2);
  } catch {
    return null;
  }
}

/** A fenced block with a title bar (its language) and a copy button; JSON is laid out and colored. */
function CodeBlock({ children }: { children?: ReactNode }) {
  const t = useT();
  const [copied, setCopied] = useState(false);
  const code = Children.toArray(children).find((child) => isValidElement(child));
  const className = isValidElement<{ className?: string }>(code) ? code.props.className : undefined;
  const language = /language-(\w+)/.exec(className ?? '')?.[1]?.toLowerCase() ?? '';
  const source = textOf(children).replace(/\n$/, '');
  const json = prettyJson(language, source);
  const shown = json ?? source;

  function copy() {
    void navigator.clipboard.writeText(shown).then(
      () => {
        setCopied(true);
        setTimeout(() => {
          setCopied(false);
        }, 1500);
      },
      () => undefined,
    );
  }

  return (
    <div className={styles.block}>
      <div className={styles.blockBar}>
        <span>{json ? 'JSON' : language || 'Code'}</span>
        <button type="button" className={styles.copy} onClick={copy}>
          {copied ? t('markdown.copied') : t('terminal.copy')}
        </button>
      </div>
      <pre>
        <code>{json ? highlightJson(json) : source}</code>
      </pre>
    </div>
  );
}

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
          pre: ({ children: block }) => <CodeBlock>{block}</CodeBlock>,
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
