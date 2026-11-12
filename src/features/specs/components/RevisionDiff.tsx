import { useEffect, useState } from 'react';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import type { RevisionDiffDto } from '@/lib/tauri/commands';
import { specRevisionDiff } from '../services/specService';
import styles from './Specs.module.css';

interface Props {
  specId: string;
  artifactId: string;
  from: number;
  to: number;
}

type State =
  | { name: 'loading' }
  | { name: 'ready'; diff: RevisionDiffDto }
  | { name: 'error'; message: string };

/**
 * What changed in a document from one revision to another. The core works it out (the same
 * answer on every screen); this only shows it, file by file.
 */
export function RevisionDiff({ specId, artifactId, from, to }: Props) {
  const { t } = useI18n();
  const [state, setState] = useState<State>({ name: 'loading' });

  useEffect(() => {
    let current = true;
    setState({ name: 'loading' });
    specRevisionDiff(specId, artifactId, from, to)
      .then((diff) => {
        if (current) setState({ name: 'ready', diff });
      })
      .catch((cause: unknown) => {
        if (current) setState({ name: 'error', message: errorMessage(t, cause) });
      });
    return () => {
      current = false;
    };
  }, [specId, artifactId, from, to, t]);

  if (state.name === 'loading') return <p className={styles.muted}>{t('specs.diff.loading')}</p>;
  if (state.name === 'error') {
    return (
      <p role="alert" className={styles.muted}>
        {state.message}
      </p>
    );
  }
  return (
    <div
      className={styles.diff}
      aria-label={t('specs.diff.label', { from: state.diff.from, to: state.diff.to })}
    >
      {state.diff.files.map((file) => (
        <section key={file.path} className={styles.diffFile}>
          <h4 className={styles.mono}>
            {file.path}{' '}
            <span className={styles.muted}>
              {t(`specs.diff.change.${file.change}`)}
              {file.change !== 'unchanged' &&
                ` · ${t('specs.diff.counts', { added: file.added, removed: file.removed })}`}
            </span>
          </h4>
          {file.tooLarge && <p className={styles.muted}>{t('specs.diff.tooLarge')}</p>}
          {file.lines.length > 0 && (
            <pre className={styles.source}>
              {file.lines.map((line, index) =>
                line.kind === 'gap' ? (
                  <span key={index} className={styles.diffGap}>
                    {t('specs.diff.gap', { lines: line.lines })}
                    {'\n'}
                  </span>
                ) : (
                  <span
                    key={index}
                    className={
                      line.kind === 'added'
                        ? styles.diffAdded
                        : line.kind === 'removed'
                          ? styles.diffRemoved
                          : undefined
                    }
                  >
                    {line.kind === 'added' ? '+ ' : line.kind === 'removed' ? '- ' : '  '}
                    {line.text}
                    {'\n'}
                  </span>
                ),
              )}
            </pre>
          )}
        </section>
      ))}
    </div>
  );
}
