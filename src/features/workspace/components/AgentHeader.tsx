import { Icon } from '@/components/ui/Icon';
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
 * Identity first: who the agent is, what it runs on, and what it is doing right now. Clicking
 * the identity and status area opens the agent's details and usage.
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
      {onToggle && (
        <button
          type="button"
          className={styles.chevron}
          aria-expanded={expanded}
          aria-label={t(expanded ? 'agent.collapse' : 'agent.expand', { name })}
          title={t(expanded ? 'agent.collapse' : 'agent.expand', { name })}
          onClick={onToggle}
        >
          <Icon name="chevronDown" size={18} />
        </button>
      )}
      <button
        type="button"
        className={styles.identity}
        aria-label={t('workspace.agentDetails', { name })}
        onClick={onOpenDetails}
      >
        <h3 className={styles.title}>{name}</h3>
        <StatusBadge status={status} />
        <span className={styles.meta}>
          {personalityName} · {runtimeName}
          {providerName ? ` · ${providerName}` : ''}
        </span>
        <span className={styles.metaModel}>{modelId}</span>
        <span
          className={styles.metaChip}
          title={t(gitIsolation ? 'agent.gitIsolation.onHint' : 'agent.gitIsolation.offHint')}
        >
          {t('agent.gitIsolation')}:{' '}
          {t(gitIsolation ? 'agent.gitIsolation.on' : 'agent.gitIsolation.off')}
        </span>
      </button>
      {onToggle && expanded === false && (
        <span className={styles.foldedActions}>
          <button type="button" className={styles.foldedButton} onClick={onToggle}>
            {t('agent.expandDetails')}
            <Icon name="chevronDown" size={14} />
          </button>
          {onOpenTerminal && (
            <button type="button" className={styles.foldedButton} onClick={onOpenTerminal}>
              {t('agent.tab.terminal')}
            </button>
          )}
        </span>
      )}
      <span className={styles.headerActions}>
        <button
          type="button"
          className={styles.remove}
          aria-label={t('workspace.editAgent', { name })}
          title={t('workspace.editAgent', { name })}
          onClick={onEdit}
        >
          ✎
        </button>
        <button
          type="button"
          className={styles.remove}
          aria-label={t('workspace.removeAgent', { name })}
          title={t('workspace.removeAgent', { name })}
          onClick={onRemove}
        >
          ✕
        </button>
      </span>
    </header>
  );
}
