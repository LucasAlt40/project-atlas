import { useState } from 'react';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { OverflowMenu } from '@/components/ui/OverflowMenu';
import type { TranslationKey } from '@/i18n';
import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import { ExportDialog } from '@/features/portability';
import type { Agent, Personality } from '../types';
import styles from './Panels.module.css';

interface Props {
  agents: Agent[];
  personalities: Personality[];
  runtimeName: (runtimeId: string) => string;
  /** Whether the agent's runtime can run it now; unknown while the runtimes are still detected. */
  runtimeReady?: (runtimeId: string) => boolean | undefined;
  onEdit: (agentId: string) => void;
  /** Rejects when the agent cannot be deleted (for example it is working). */
  onDelete: (agentId: string) => Promise<void>;
  /** The list is narrowed by a search, so an empty list means "no match". */
  filtered?: boolean;
  /** Offered in the empty state, when there is no agent at all. */
  onCreate?: () => void;
}

/** Every saved agent, whether or not it is in a workspace. */
export function AgentList({
  agents,
  personalities,
  runtimeName,
  runtimeReady = () => undefined,
  onEdit,
  onDelete,
  filtered = false,
  onCreate,
}: Props) {
  const t = useT();
  const [confirming, setConfirming] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [exporting, setExporting] = useState<Agent | null>(null);

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
    if (filtered) return <p className={styles.muted}>{t('agents.noMatch')}</p>;
    return (
      <div className={styles.emptyState}>
        <p>{t('agents.empty')}</p>
        {onCreate && (
          <Button onClick={onCreate}>
            <Icon name="plus" size={14} />
            {t('agents.create')}
          </Button>
        )}
      </div>
    );
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
          const ready = runtimeReady(agent.runtimeId);
          return (
            <li key={agent.id} className={styles.agentCard}>
              <header className={styles.agentCardHead}>
                <span className={styles.agentIcon} aria-hidden="true">
                  <Icon name="agents" size={20} />
                </span>
                <div className={styles.agentCardTitle}>
                  <div className={styles.agentNameRow}>
                    <strong className={styles.agentCardName}>{agent.name}</strong>
                    <Badge mono caps={false}>
                      {personality?.name ?? agent.personalityId}
                    </Badge>
                  </div>
                  <span className={styles.agentRuntime}>
                    {t('agents.card.runtime')}: <b>{runtimeName(agent.runtimeId)}</b>
                  </span>
                </div>
                {ready !== undefined && (
                  <Badge dot mono tone={ready ? 'success' : 'warning'}>
                    {t(ready ? 'agent.status.ready' : 'agent.status.unavailable')}
                  </Badge>
                )}
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
                    <Button
                      variant="danger"
                      onClick={() => {
                        confirmDelete(agent.id);
                      }}
                    >
                      {t('common.confirmDelete')}
                    </Button>
                    <Button
                      variant="ghost"
                      onClick={() => {
                        setConfirming(null);
                      }}
                    >
                      {t('common.cancel')}
                    </Button>
                  </span>
                ) : (
                  <span className={styles.iconActions}>
                    <Button
                      variant="secondary"
                      aria-label={t('workspace.editAgent', { name: agent.name })}
                      onClick={() => {
                        onEdit(agent.id);
                      }}
                    >
                      {t('common.edit')}
                    </Button>
                    <OverflowMenu
                      label={t('agent.moreActions', { name: agent.name })}
                      items={[
                        {
                          label: t('portability.export'),
                          icon: 'upload',
                          onSelect: () => {
                            setExporting(agent);
                          },
                        },
                        {
                          label: t('common.delete'),
                          icon: 'trash',
                          danger: true,
                          onSelect: () => {
                            setConfirming(agent.id);
                          },
                        },
                      ]}
                    />
                  </span>
                )}
              </footer>
            </li>
          );
        })}
      </ul>
      {exporting && (
        <ExportDialog
          targets={[{ kind: 'agent', id: exporting.id }]}
          title={t('portability.export.title', { name: exporting.name })}
          onClose={() => {
            setExporting(null);
          }}
        />
      )}
    </>
  );
}
