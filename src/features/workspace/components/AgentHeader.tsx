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
  status: AgentStatus;
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
  status,
  onOpenDetails,
  onEdit,
  onRemove,
}: Props) {
  const t = useT();
  return (
    <header className={styles.header}>
      <div className={styles.titleRow}>
        <button
          type="button"
          className={styles.identity}
          aria-label={t('workspace.agentDetails', { name })}
          onClick={onOpenDetails}
        >
          <h3 className={styles.title}>{name}</h3>
          <span className={styles.meta}>
            {personalityName} · {runtimeName}
            {providerName ? ` · ${providerName}` : ''}
          </span>
          <span className={styles.meta}>{modelId}</span>
          <StatusBadge status={status} />
        </button>
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
      </div>
    </header>
  );
}
