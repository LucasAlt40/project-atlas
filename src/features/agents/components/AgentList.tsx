import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { OverflowMenu } from '@/components/ui/OverflowMenu';
import { StateBadge } from '@/components/ui/status';
import type { TranslationKey } from '@/i18n';
import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import { ExportDialog } from '@/features/portability';
import type { Agent, Personality, RuntimeStatus } from '../types';
import styles from './Panels.module.css';

interface Props {
  agents: Agent[];
  personalities: Personality[];
  runtimeName: (runtimeId: string) => string;
  /** Whether the agent's runtime can run it now; unknown while the runtimes are still detected. */
  runtimeReady?: (runtimeId: string) => boolean | undefined;
  /** Why a runtime cannot run now (not installed, not signed in), when it cannot. */
  runtimeAvailability?: (runtimeId: string) => RuntimeStatus['availability'] | undefined;
  onEdit: (agentId: string) => void;
  /** Rejects when the agent cannot be deleted (for example it is working). */
  onDelete: (agentId: string) => Promise<void>;
  /** The list is narrowed by a search, so an empty list means "no match". */
  filtered?: boolean;
  /** Offered in the empty state, when there is no agent at all. */
  onCreate?: () => void;
}

/** Every saved agent, whether or not it is in a workspace: one row each, its actions at the end. */
export function AgentList({
  agents,
  personalities,
  runtimeName,
  runtimeReady = () => undefined,
  runtimeAvailability = () => undefined,
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
            <Icon name="plus" size={16} />
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
      <div className={styles.tableWrap}>
        <table className={styles.table} aria-label={t('agents.list')}>
          <thead>
            <tr>
              <th scope="col">{t('agents.col.agent')}</th>
              <th scope="col">{t('agents.col.runtime')}</th>
              <th scope="col">{t('agents.col.model')}</th>
              <th scope="col">{t('agents.col.permissions')}</th>
              <th scope="col">{t('agents.col.result')}</th>
              <th scope="col">{t('agents.col.isolation')}</th>
              <th scope="col" className={styles.actionsCol}>
                {t('agents.col.actions')}
              </th>
            </tr>
          </thead>
          <tbody>
            {agents.map((agent) => {
              const personality = personalities.find((p) => p.id === agent.personalityId);
              const profile = agent.permissionProfileId ?? 'read_only';
              const ready = runtimeReady(agent.runtimeId);
              const availability = runtimeAvailability(agent.runtimeId);
              const outcomes = agent.resultContract.outcomes.map((o) => o.id).join(' / ');
              return (
                <tr key={agent.id} aria-label={agent.name}>
                  <td>
                    <span className={styles.agentName}>{agent.name}</span>
                    <span className={styles.cellSub}>
                      {personality?.name ?? agent.personalityId}
                    </span>
                  </td>
                  <td>
                    <span className={styles.runtimeCell}>
                      {runtimeName(agent.runtimeId)}
                      {ready !== undefined && (
                        <StateBadge state={ready ? 'ready' : 'unavailable'}>
                          {t(ready ? 'agent.status.ready' : 'agent.status.unavailable')}
                        </StateBadge>
                      )}
                    </span>
                    {ready === false && availability && availability !== 'ready' && (
                      <span className={styles.cellSub}>
                        {t(`agents.unavailable.${availability}` as TranslationKey)}
                      </span>
                    )}
                  </td>
                  <td className={styles.mono}>{agent.modelId || '—'}</td>
                  <td>{t(`security.profile.${profile}` as TranslationKey)}</td>
                  <td {...(outcomes ? { title: outcomes } : {})}>
                    {t(`contract.kind.${agent.resultContract.kind}`)}
                  </td>
                  <td className={styles.mono}>
                    {t(
                      agent.worktreeIsolation
                        ? 'agents.isolation.worktree'
                        : 'agents.isolation.none',
                    )}
                  </td>
                  <td className={styles.actionsCell}>
                    {confirming === agent.id ? (
                      <span
                        role="group"
                        aria-label={t('personalities.deleteQuestion', { name: agent.name })}
                        className={styles.confirm}
                      >
                        <span>{t('agents.deleteConfirm', { name: agent.name })}</span>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => {
                            setConfirming(null);
                          }}
                        >
                          {t('common.cancel')}
                        </Button>
                        <Button
                          variant="danger"
                          size="sm"
                          onClick={() => {
                            confirmDelete(agent.id);
                          }}
                        >
                          {t('common.confirmDelete')}
                        </Button>
                      </span>
                    ) : (
                      <span className={styles.rowActions}>
                        <Button
                          variant="secondary"
                          size="sm"
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
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
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
