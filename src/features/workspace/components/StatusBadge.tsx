import { useT } from '@/i18n/I18nProvider';
import type { AgentStatus } from '../model/agentStatus';
import styles from './AgentCard.module.css';

export function StatusBadge({ status }: { status: AgentStatus }) {
  const t = useT();
  return (
    <span className={styles.status} data-status={status} role="status">
      <span aria-hidden="true">●</span> {t(`agent.status.${status}`)}
    </span>
  );
}
