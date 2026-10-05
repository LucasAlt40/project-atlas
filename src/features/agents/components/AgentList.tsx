import { useState } from 'react';
import { Icon } from '@/components/ui/Icon';
import type { TranslationKey } from '@/i18n';
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
  /** The list is narrowed by a search, so an empty list means "no match". */
  filtered?: boolean;
}

/** Every saved agent, whether or not it is in a workspace. */
export function AgentList({
  agents,
  personalities,
  runtimeName,
  onEdit,
  onDelete,
  filtered = false,
}: Props) {
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
    return <p className={styles.muted}>{t(filtered ? 'agents.noMatch' : 'agents.empty')}</p>;
  }
  return (
    <>
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
      <ul className={styles.agentGrid} aria-label={t('agents.list')}>
        {agents.map((agent) => {
          const personality = personalities.find((p) => p.id === agent.personalityId);
          const profile = agent.permissionProfileId ?? 'read_only';
          const rows: [string, string][] = [
            [t('agents.card.model'), agent.modelId || '—'],
            [t('agents.card.permissions'), t(`security.profile.${profile}` as TranslationKey)],
            [
              t('agents.card.result'),
              agent.resultContract.outcomes.length > 0
                ? agent.resultContract.outcomes.map((o) => o.id).join(' / ')
                : t('agents.card.noResult'),
            ],
            [
              t('agents.card.isolation'),
              t(agent.worktreeIsolation ? 'agent.gitIsolation.on' : 'agent.gitIsolation.off'),
            ],
          ];
          return (
            <li key={agent.id} className={styles.agentCard}>
              <header className={styles.agentCardHead}>
                <span className={styles.agentIcon} aria-hidden="true">
                  <Icon name="agents" size={22} />
                </span>
                <div className={styles.agentCardTitle}>
                  <div className={styles.agentNameRow}>
                    <strong className={styles.agentCardName}>{agent.name}</strong>
                    <span className={styles.chip}>{personality?.name ?? agent.personalityId}</span>
                  </div>
                  <span className={styles.agentRuntime}>{runtimeName(agent.runtimeId)}</span>
                </div>
              </header>
              <dl className={styles.agentFacts}>
                {rows.map(([label, value]) => (
                  <div key={label}>
                    <dt>{label}</dt>
                    <dd>{value}</dd>
                  </div>
                ))}
              </dl>
              <footer className={styles.agentCardFoot}>
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
                  <span className={styles.iconActions}>
                    <button
                      type="button"
                      className={styles.iconButton}
                      aria-label={t('workspace.editAgent', { name: agent.name })}
                      title={t('common.edit')}
                      onClick={() => {
                        onEdit(agent.id);
                      }}
                    >
                      <Icon name="edit" size={16} />
                    </button>
                    <button
                      type="button"
                      className={[styles.iconButton, styles.iconDanger].join(' ')}
                      aria-label={t('workspace.manage.deleteLabel', { name: agent.name })}
                      title={t('common.delete')}
                      onClick={() => {
                        setConfirming(agent.id);
                      }}
                    >
                      <Icon name="trash" size={16} />
                    </button>
                  </span>
                )}
              </footer>
            </li>
          );
        })}
      </ul>
    </>
  );
}
