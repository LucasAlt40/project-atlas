import { useMemo } from 'react';
import { useI18n } from '@/i18n/I18nProvider';
import type { FileChangeDto } from '@/lib/tauri/commands';
import { STATUS_LABEL, STATUS_LETTER, treeRows, type ActivityEntry } from '../model/live';
import styles from './LiveWorkspace.module.css';

/** The files that really differ from the baseline, as a tree. Everything shown is the backend's. */
export function LiveFileTree({
  files,
  selected,
  collapsed,
  onSelect,
  onToggle,
}: {
  files: readonly FileChangeDto[];
  /** `null`: all changes. */
  selected: string | null;
  collapsed: ReadonlySet<string>;
  onSelect: (path: string | null) => void;
  onToggle: (folder: string) => void;
}) {
  const { t } = useI18n();
  const rows = useMemo(() => treeRows(files, collapsed), [files, collapsed]);
  return (
    <div className={styles.tree} role="group" aria-label={t('live.files')}>
      <button
        type="button"
        className={styles.treeRow}
        aria-pressed={selected === null}
        onClick={() => {
          onSelect(null);
        }}
      >
        <span className={styles.treeName}>{t('live.files.all')}</span>
        <span className={styles.muted}>{files.length}</span>
      </button>
      {files.length === 0 && <p className={styles.note}>{t('live.files.empty')}</p>}
      {rows.map((row) =>
        row.kind === 'dir' ? (
          <button
            key={`d:${row.path}`}
            type="button"
            className={styles.treeRow}
            style={{ paddingLeft: `${String(0.5 + row.depth * 0.9)}rem` }}
            aria-expanded={!collapsed.has(row.path)}
            onClick={() => {
              onToggle(row.path);
            }}
          >
            <span aria-hidden="true" className={styles.caret}>
              {collapsed.has(row.path) ? '▸' : '▾'}
            </span>
            <span className={styles.treeName}>{row.name}</span>
            <span className={styles.muted}>{row.count}</span>
          </button>
        ) : (
          <button
            key={`f:${row.path}`}
            type="button"
            className={styles.treeRow}
            style={{ paddingLeft: `${String(1.4 + row.depth * 0.9)}rem` }}
            aria-pressed={selected === row.path}
            aria-label={`${row.path} (${row.change ? t(STATUS_LABEL[row.change.status]) : ''})`}
            title={
              row.change?.oldPath
                ? t('live.files.renamedFrom', { path: row.change.oldPath })
                : row.path
            }
            onClick={() => {
              onSelect(row.path);
            }}
          >
            <span className={styles.status} data-status={row.change?.status} aria-hidden="true">
              {row.change ? STATUS_LETTER[row.change.status] : ''}
            </span>
            <span className={styles.treeName} data-deleted={row.change?.status === 'deleted'}>
              {row.name}
            </span>
            {row.change?.additions !== null && row.change?.additions !== undefined && (
              <span className={styles.counts} aria-hidden="true">
                <span data-sign="add">+{row.change.additions}</span>{' '}
                <span data-sign="del">−{row.change.deletions ?? 0}</span>
              </span>
            )}
          </button>
        ),
      )}
    </div>
  );
}

const time = (at: number) =>
  new Date(at).toLocaleTimeString(undefined, {
    hour12: false,
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });

/** What really happened to the worktree, newest first: files that changed and phases that began. */
export function LiveActivity({
  entries,
  onOpen,
}: {
  entries: readonly ActivityEntry[];
  onOpen: (path: string) => void;
}) {
  const { t } = useI18n();
  return (
    <div className={styles.activity}>
      <h4 className={styles.sectionTitle}>{t('live.activity')}</h4>
      {entries.length === 0 ? (
        <p className={styles.note}>{t('live.activity.empty')}</p>
      ) : (
        <ul className={styles.activityList} aria-label={t('live.activity')}>
          {entries.map((entry) => (
            <li key={entry.id} className={styles.activityItem}>
              <time className={styles.muted}>{time(entry.at)}</time>
              {entry.kind === 'phase' ? (
                <span className={styles.activityPhase}>{t(`live.phase.${entry.phase}`)}</span>
              ) : (
                <>
                  <button
                    type="button"
                    className={styles.linkButton}
                    onClick={() => {
                      onOpen(entry.path);
                    }}
                  >
                    {entry.path}
                  </button>
                  <span
                    className={styles.activityStatus}
                    data-status={entry.kind === 'file' ? entry.status : 'reverted'}
                  >
                    {entry.kind === 'file'
                      ? t(STATUS_LABEL[entry.status]).toLowerCase()
                      : t('live.activity.reverted')}
                  </span>
                </>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
