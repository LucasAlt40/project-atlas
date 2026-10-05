import { useEffect, useMemo, useRef } from 'react';
import { useI18n } from '@/i18n/I18nProvider';
import type { LiveFileDto } from '@/lib/tauri/commands';
import { parseDiff } from '../model/diff';
import { familyOf, highlightLine, START, type LineState, type Token } from '../model/highlight';
import styles from './LiveWorkspace.module.css';

function Tokens({ tokens }: { tokens: Token[] }) {
  return (
    <>
      {tokens.map((token, index) => (
        <span key={String(index)} data-token={token.kind}>
          {token.text}
        </span>
      ))}
    </>
  );
}

/** A file's text with line numbers and syntax colors. Read only. */
export function CodeView({ file }: { file: LiveFileDto }) {
  const { t } = useI18n();
  const lines = useMemo(() => {
    const family = familyOf(file.path);
    const colored: Token[][] = [];
    let state: LineState = START;
    for (const line of (file.content ?? '').split('\n')) {
      const result = highlightLine(line, family, state);
      state = result.state;
      colored.push(result.tokens);
    }
    return colored;
  }, [file.path, file.content]);
  // A trailing newline is the end of the last line, not an empty line of its own.
  const shown = lines.length > 1 && file.content?.endsWith('\n') ? lines.slice(0, -1) : lines;

  if (file.kind === 'deleted') return <p className={styles.note}>{t('live.file.deleted')}</p>;
  if (file.kind === 'binary') return <p className={styles.note}>{t('live.file.binary')}</p>;
  if (file.kind === 'symlink') return <p className={styles.note}>{t('live.file.symlink')}</p>;
  if (file.kind === 'not_file') return <p className={styles.note}>{t('live.file.notFile')}</p>;
  return (
    <>
      {file.truncated && (
        <p className={styles.note} role="status">
          {t('live.file.truncated', { kb: Math.round((file.content?.length ?? 0) / 1024) })}
        </p>
      )}
      <pre className={styles.code} aria-label={t('live.file.content', { path: file.path })}>
        {shown.map((tokens, index) => (
          <span key={String(index)} className={styles.codeLine}>
            <span className={styles.lineNo} aria-hidden="true">
              {index + 1}
            </span>
            <span className={styles.lineText}>
              <Tokens tokens={tokens} />
              {'\n'}
            </span>
          </span>
        ))}
      </pre>
    </>
  );
}

/** A unified diff with both line numbers, and a way to jump between its changes. */
export function DiffView({ text, path }: { text: string; path: string | null }) {
  const { t } = useI18n();
  const parsed = useMemo(() => parseDiff(text), [text]);
  const rows = useRef<(HTMLSpanElement | null)[]>([]);
  const at = useRef(-1);
  // A different diff starts from its first change again.
  useEffect(() => {
    at.current = -1;
  }, [text, path]);

  if (text.trim() === '') return <p className={styles.note}>{t('live.diff.empty')}</p>;

  const go = (step: 1 | -1) => {
    const starts = parsed.changeStarts;
    if (starts.length === 0) return;
    const next = (at.current + step + starts.length) % starts.length;
    at.current = next;
    const row = starts[next];
    rows.current[row ?? 0]?.scrollIntoView({ block: 'center' });
  };

  return (
    <>
      <div className={styles.diffBar}>
        <span className={styles.muted}>
          <span data-sign="add">+{parsed.additions}</span>{' '}
          <span data-sign="del">−{parsed.deletions}</span>
        </span>
        {parsed.changeStarts.length > 1 && (
          <span className={styles.diffNav}>
            <button
              type="button"
              className={styles.linkButton}
              onClick={() => {
                go(-1);
              }}
            >
              ↑ {t('live.diff.previous')}
            </button>
            <button
              type="button"
              className={styles.linkButton}
              onClick={() => {
                go(1);
              }}
            >
              ↓ {t('live.diff.next')}
            </button>
          </span>
        )}
      </div>
      <pre className={styles.diff} aria-label={t('live.diff.label')}>
        {parsed.rows.map((row, index) => (
          <span
            key={String(index)}
            className={styles.diffRow}
            data-kind={row.kind}
            ref={(element) => {
              rows.current[index] = element;
            }}
          >
            <span className={styles.lineNo} aria-hidden="true">
              {row.oldNo ?? ''}
            </span>
            <span className={styles.lineNo} aria-hidden="true">
              {row.newNo ?? ''}
            </span>
            <span className={styles.diffMark} aria-hidden="true">
              {row.kind === 'add' ? '+' : row.kind === 'del' ? '−' : ' '}
            </span>
            <span className={styles.lineText}>
              {row.text}
              {'\n'}
            </span>
          </span>
        ))}
      </pre>
    </>
  );
}
