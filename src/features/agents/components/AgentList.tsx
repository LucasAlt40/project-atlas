import { useState } from 'react';
import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import type { Agent, Personality } from '../types';
import styles from './Panels.module.css';

interface Props {
  agents: Agent[];
  personalities: Personality[];
  runtimeName: (runtimeId: string) => string;
  onEdit: (agentId: string) => void;
  /** Rejects when the agent cannot be deleted (for example it is working). */
  onDelete: (agentId: string) => Promise<void>;
}

/** Every saved agent, whether or not it is in a workspace. */
export function AgentList({ agents, personalities, runtimeName, onEdit, onDelete }: Props) {
  const t = useT();
  const [confirming, setConfirming] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  function confirmDelete(agentId: string) {
    setError(null);
    onDelete(agentId)
      .catch((e: unknown) => {
        setError(errorMessage(t, e));
      })
      .finally(() => {
        setConfirming(null);
      });
  }

  if (agents.length === 0) {
    return <p className={styles.muted}>{t('agents.empty')}</p>;
  }
  return (
    <>
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
      <ul className={styles.agentList} aria-label={t('agents.list')}>
        {agents.map((agent) => {
          const personality = personalities.find((p) => p.id === agent.personalityId);
          return (
            <li key={agent.id} className={styles.agentRow}>
              <div>
                <strong>{agent.name}</strong>
                <p className={styles.muted}>
                  {personality?.name ?? agent.personalityId} · {runtimeName(agent.runtimeId)} ·{' '}
                  {agent.modelId}
                </p>
              </div>
              <div className={styles.rowActions}>
                <button
                  type="button"
                  className={styles.linkButton}
                  aria-label={t('workspace.editAgent', { name: agent.name })}
                  onClick={() => {
                    onEdit(agent.id);
                  }}
                >
                  {t('common.edit')}
                </button>
                {confirming === agent.id ? (
                  <span
                    role="group"
                    aria-label={t('personalities.deleteQuestion', { name: agent.name })}
                    className={styles.confirm}
                  >
                    {t('agents.deleteConfirm', { name: agent.name })}
                    <button
                      type="button"
                      className={styles.danger}
                      onClick={() => {
                        confirmDelete(agent.id);
                      }}
                    >
                      {t('common.confirmDelete')}
                    </button>
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
                    aria-label={t('workspace.manage.deleteLabel', { name: agent.name })}
                    onClick={() => {
                      setConfirming(agent.id);
                    }}
                  >
                    {t('common.delete')}
                  </button>
                )}
              </div>
            </li>
          );
        })}
      </ul>
    </>
  );
}
