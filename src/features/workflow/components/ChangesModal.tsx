import { useEffect, useState } from 'react';
import { Modal } from '@/components/ui/Modal';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import { getChanges, getDiff } from '../services/workflowService';
import type { ChangeSet, WorkflowRun } from '../types';
import { FileLine } from './HandoffView';
import styles from './Workflow.module.css';

type Diff =
  { status: 'loading' } | { status: 'error'; error: unknown } | { status: 'ready'; text: string };

/** A diff and what it was asked for, so a diff of another file is never shown as this one's. */
interface Loaded {
  key: string;
  diff: Diff;
}

/** The real diff of the run's code, file by file, read from Git when asked for. */
export function ChangesModal({
  run,
  initialFile = null,
  onClose,
}: {
  run: WorkflowRun;
  /** The file whose diff opens first; all of them when absent. */
  initialFile?: string | null;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const [changes, setChanges] = useState<ChangeSet | null>(run.changes);
  const [file, setFile] = useState<string | null>(initialFile);
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const key = `${run.id}|${file ?? ''}`;
  const diff: Diff = loaded?.key === key ? loaded.diff : { status: 'loading' };

  useEffect(() => {
    let cancelled = false;
    getChanges(run.id)
      .then((fresh) => {
        if (!cancelled && fresh) setChanges(fresh);
      })
      .catch(() => {
        // The summary kept with the run is enough to list the files.
      });
    return () => {
      cancelled = true;
    };
  }, [run.id]);

  useEffect(() => {
    let cancelled = false;
    getDiff(run.id, file ?? undefined)
      .then((text) => {
        if (!cancelled) setLoaded({ key, diff: { status: 'ready', text } });
      })
      .catch((error: unknown) => {
        if (!cancelled) setLoaded({ key, diff: { status: 'error', error } });
      });
    return () => {
      cancelled = true;
    };
  }, [run.id, file, key]);

  return (
    <Modal label={t('changes.title')} onClose={onClose} size="wide">
      <h2>{t('changes.title')}</h2>
      {changes && (
        <p className={styles.muted}>
          {t('integration.stats', {
            files: changes.filesChanged,
            additions: changes.additions,
            deletions: changes.deletions,
          })}
        </p>
      )}
      <div className={styles.changesLayout}>
        <ul className={styles.fileList} aria-label={t('changes.files')}>
          <li>
            <button
              type="button"
              className={styles.link}
              aria-pressed={file === null}
              onClick={() => {
                setFile(null);
              }}
            >
              {t('changes.all')}
            </button>
          </li>
          {(changes?.files ?? []).map((f) => (
            <li key={f.path}>
              <button
                type="button"
                className={styles.fileButton}
                aria-pressed={file === f.path}
                aria-label={t('changes.open', { path: f.path })}
                onClick={() => {
                  setFile(f.path);
                }}
              >
                <FileLine file={f} />
              </button>
            </li>
          ))}
        </ul>
        <div className={styles.diff} aria-label={t('changes.diff')}>
          {diff.status === 'loading' && <p className={styles.muted}>{t('common.loadingCore')}</p>}
          {diff.status === 'error' && (
            <p role="alert" className={styles.failure}>
              {errorMessage(t, diff.error)}
            </p>
          )}
          {diff.status === 'ready' &&
            (diff.text.trim() === '' ? (
              <p className={styles.muted}>{t('changes.empty')}</p>
            ) : (
              <pre className={styles.diffText}>
                {diff.text.split('\n').map((line, index) => (
                  <span key={String(index)} className={styles.diffLine} data-kind={lineKind(line)}>
                    {line}
                    {'\n'}
                  </span>
                ))}
              </pre>
            ))}
        </div>
      </div>
    </Modal>
  );
}

function lineKind(line: string): 'add' | 'del' | 'hunk' | 'meta' | 'ctx' {
  if (line.startsWith('+++') || line.startsWith('---') || line.startsWith('diff ')) return 'meta';
  if (line.startsWith('@@')) return 'hunk';
  if (line.startsWith('+')) return 'add';
  if (line.startsWith('-')) return 'del';
  return 'ctx';
}
