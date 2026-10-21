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
  /** Folded in the list: reach the terminal without unfolding by hand (from "⋯"). */
  onOpenTerminal?: () => void;
  /** `grid`: one compact line (name, state, model, ⋯); `list`: the full header. */
  variant?: 'grid' | 'list';
  onOpenDetails: () => void;
  onEdit: () => void;
  onRemove: () => void;
}

/**
 * Identity first: who the agent is and what it is doing right now, then what it runs on in one
 * quiet line. Clicking the name opens the agent's details; "⋯" keeps the rare actions.
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
  variant = 'list',
  onOpenDetails,
  onEdit,
  onRemove,
}: Props) {
  const t = useT();
  const compact = variant === 'grid';
  const lineage = [personalityName, runtimeName, providerName].filter(Boolean).join(' · ');
  return (
    <header className={styles.header} data-compact={compact ? true : undefined}>
      <div className={styles.identityRow}>
        <button
          type="button"
          className={styles.identity}
          aria-label={t('workspace.agentDetails', { name })}
          onClick={onOpenDetails}
        >
          <h3 className={styles.title}>{name}</h3>
        </button>
        <StatusBadge status={status} />
        {!compact && (
          <>
            <span className={styles.rule} aria-hidden="true" />
            <span className={styles.lineage} title={lineage}>
              {lineage}
            </span>
          </>
        )}
        <span className={styles.model} title={modelId}>
          {modelId}
        </span>
        {!compact && (
          <span
            className={styles.isolation}
            data-on={gitIsolation}
            title={t(gitIsolation ? 'agent.gitIsolation.onHint' : 'agent.gitIsolation.offHint')}
          >
            {gitIsolation && <Icon name="lock" size={12} />}
            {t(gitIsolation ? 'agent.gitIsolation.badgeOn' : 'agent.gitIsolation.badgeOff')}
          </span>
        )}
      </div>
      <span className={styles.headerActions}>
        {!compact && (
          <Button
            variant="secondary"
            size="sm"
            aria-label={t('workspace.editAgent', { name })}
            onClick={onEdit}
          >
            {t('common.edit')}
          </Button>
        )}
        <OverflowMenu
          label={t('agent.moreActions', { name })}
          items={[
            ...(compact
              ? [{ label: t('common.edit'), icon: 'edit' as const, onSelect: onEdit }]
              : []),
            ...(onOpenTerminal && expanded === false
              ? [
                  {
                    label: t('agent.tab.terminal'),
                    icon: 'terminal' as const,
                    onSelect: onOpenTerminal,
                  },
                ]
              : []),
            {
              label: t('workspace.openDetails'),
              icon: 'info',
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
          <button
            type="button"
            className={styles.toggle}
            aria-expanded={expanded}
            aria-label={t(expanded ? 'agent.collapse' : 'agent.expand', { name })}
            title={t(expanded ? 'agent.collapse' : 'agent.expand', { name })}
            onClick={onToggle}
          >
            <Icon name={expanded ? 'chevronUp' : 'chevronDown'} size={18} />
          </button>
        )}
      </span>
    </header>
  );
}
