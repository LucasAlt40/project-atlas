import { useState } from 'react';
import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import type { Workspace } from '../types';
import styles from './Workspace.module.css';

interface Props {
  workspaces: Workspace[];
  onEdit: (workspace: Workspace) => void;
  /** Rejects when the workspace cannot be deleted (for example an agent is working in it). */
  onDelete: (workspaceId: string) => Promise<void>;
}

/** A plain list of workspaces to rename, repoint or delete. */
export function ManageWorkspaces({ workspaces, onEdit, onDelete }: Props) {
  const t = useT();
  const [confirming, setConfirming] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  function confirmDelete(id: string) {
    setError(null);
    onDelete(id)
      .catch((e: unknown) => {
        setError(errorMessage(t, e));
      })
      .finally(() => {
        setConfirming(null);
      });
  }

  return (
    <div className={styles.form}>
      <h2 className={styles.panelTitle}>{t('workspace.manage.title')}</h2>
      {workspaces.length === 0 && <p className={styles.muted}>{t('workspace.manage.empty')}</p>}
      {error && (
        <p role="alert" className={styles.notice}>
          {error}
        </p>
      )}
      <ul className={styles.manageList}>
        {workspaces.map((workspace) => (
          <li key={workspace.id} className={styles.manageRow}>
            <div>
              <strong>{workspace.name}</strong>
              <p className={styles.muted}>{workspace.projectPath}</p>
            </div>
            <div className={styles.panelActions}>
              <button
                type="button"
                className={styles.linkButton}
                aria-label={t('workspace.manage.editLabel', { name: workspace.name })}
                onClick={() => {
                  onEdit(workspace);
                }}
              >
                {t('common.edit')}
              </button>
              {confirming === workspace.id ? (
                <span
                  role="group"
                  aria-label={t('workspace.manage.deleteLabel', { name: workspace.name })}
                >
                  {t('workspace.manage.confirm', { name: workspace.name })}{' '}
                  <button
                    type="button"
                    className={styles.danger}
                    onClick={() => {
                      confirmDelete(workspace.id);
                    }}
                  >
                    {t('common.confirmDelete')}
                  </button>{' '}
                  <button
                    type="button"
                    className={styles.linkButton}
                    onClick={() => {
                      setConfirming(null);
                    }}
                  >
                    {t('common.cancel')}
                  </button>
                </span>
              ) : (
                <button
                  type="button"
                  className={styles.linkButton}
                  aria-label={t('workspace.manage.deleteLabel', { name: workspace.name })}
                  onClick={() => {
                    setConfirming(workspace.id);
                  }}
                >
                  {t('common.delete')}
                </button>
              )}
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
