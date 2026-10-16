import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { OverflowMenu } from '@/components/ui/OverflowMenu';
import { useT } from '@/i18n/I18nProvider';
import type { AgentStatus } from '../model/agentStatus';
import styles from './AgentCard.module.css';
import { StatusBadge } from './StatusBadge';

interface Props {
  name: string;
  personalityName: string;
  runtimeName: string;
  providerName: string | null;
  modelId: string;
  /** Each execution works in its own Git worktree. */
  gitIsolation: boolean;
  status: AgentStatus;
  /** In the expandable list: whether the body is open, and how to toggle it. */
  expanded?: boolean;
  onToggle?: () => void;
  /** Folded in the list: reach the terminal without unfolding by hand. */
  onOpenTerminal?: () => void;
  onOpenDetails: () => void;
  onEdit: () => void;
  onRemove: () => void;
}

/**
 * Identity first: who the agent is and what it is doing right now, then a strip with what it
 * runs on. Clicking the identity opens the agent's details and usage; the "⋯" menu keeps the
 * secondary and destructive actions.
 */
export function AgentHeader({
  name,
  personalityName,
  runtimeName,
  providerName,
  modelId,
  gitIsolation,
  status,
  expanded,
  onToggle,
  onOpenTerminal,
  onOpenDetails,
  onEdit,
  onRemove,
}: Props) {
  const t = useT();
  return (
    <header className={styles.header}>
      <div className={styles.headerTop}>
        <span className={styles.avatar} aria-hidden="true">
          <Icon name="agents" size={18} />
        </span>
        <button
          type="button"
          className={styles.identity}
          aria-label={t('workspace.agentDetails', { name })}
          onClick={onOpenDetails}
        >
          <h3 className={styles.title}>{name}</h3>
          <StatusBadge status={status} />
        </button>
        <span className={styles.headerActions}>
          {onToggle && expanded === false && onOpenTerminal && (
            <Button variant="ghost" onClick={onOpenTerminal}>
              {t('agent.tab.terminal')}
            </Button>
          )}
          <Button
            variant="secondary"
            aria-label={t('workspace.editAgent', { name })}
            onClick={onEdit}
          >
            {t('common.edit')}
          </Button>
          <OverflowMenu
            label={t('agent.moreActions', { name })}
            items={[
              {
                label: t('workspace.openDetails'),
                icon: 'shield',
                onSelect: onOpenDetails,
              },
              {
                label: t('workspace.removeAgent', { name }),
                icon: 'trash',
                danger: true,
                onSelect: onRemove,
              },
            ]}
          />
          {onToggle && (
            <Button
              variant="secondary"
              aria-expanded={expanded}
              aria-label={t(expanded ? 'agent.collapse' : 'agent.expand', { name })}
              onClick={onToggle}
            >
              <Icon name={expanded ? 'chevronUp' : 'chevronDown'} size={14} />
              {t(expanded ? 'agent.collapseShort' : 'agent.expandShort')}
            </Button>
          )}
        </span>
      </div>
      <p className={styles.metaStrip}>
        <span>
          {t('agent.meta.personality')}: <b>{personalityName}</b>
        </span>
        <span>
          {t('agent.meta.runtime')}: <b>{runtimeName}</b>
        </span>
        {providerName && (
          <span>
            {t('agent.meta.provider')}: <b>{providerName}</b>
          </span>
        )}
        <span>
          {t('agent.meta.model')}: <b className={styles.metaModel}>{modelId}</b>
        </span>
        <span
          className={styles.metaGit}
          data-on={gitIsolation}
          title={t(gitIsolation ? 'agent.gitIsolation.onHint' : 'agent.gitIsolation.offHint')}
        >
          <span aria-hidden="true">●</span> {t('agent.gitIsolation')}:{' '}
          <b>{t(gitIsolation ? 'agent.gitIsolation.on' : 'agent.gitIsolation.off')}</b>
        </span>
      </p>
    </header>
  );
}
