import { Icon } from '@/components/ui/Icon';
import { useI18n } from '@/i18n/I18nProvider';
import { agoText } from '@/features/workflow/model/time';
import { phaseLabel } from '../model/progress';
import { statusView } from '../model/status';
import type { SpecSummary } from '../types';
import { StatusBadge } from './StatusBadge';
import styles from './Specs.module.css';

interface Props {
  spec: SpecSummary;
  agentName: string;
  now: number;
  onOpen: (specId: string) => void;
}

/** One spec in the overview: where it is, how far, and whether it is waiting for the person. */
export function SpecCard({ spec, agentName, now, onOpen }: Props) {
  const { t, language } = useI18n();
  const status = statusView(spec.status);
  return (
    <button
      type="button"
      className={styles.card}
      data-status={spec.status}
      onClick={() => {
        onOpen(spec.id);
      }}
      aria-label={spec.title}
    >
      <span className={styles.cardTop}>
        <span className={styles.mono}>{spec.changeId}</span>
        <StatusBadge tone={status.tone}>{t(status.key)}</StatusBadge>
      </span>
      <span className={styles.cardTitle}>{spec.title}</span>
      <span className={styles.excerpt}>{spec.request}</span>
      <span className={styles.progressRow}>
        <span
          className={styles.progress}
          role="progressbar"
          aria-label={t('specs.progress')}
          aria-valuemin={0}
          aria-valuemax={spec.stageTotal}
          aria-valuenow={spec.approved}
        >
          <span
            className={styles.progressFill}
            style={{ width: `${String((spec.approved / Math.max(spec.stageTotal, 1)) * 100)}%` }}
          />
        </span>
        <span className={styles.progressText}>
          {t('specs.approvedOf', { approved: spec.approved, total: spec.stageTotal })}
        </span>
      </span>
      <span className={styles.cardFoot}>
        <span className={styles.footItem}>
          <Icon name="agents" size={13} />
          {agentName}
        </span>
        <span className={styles.footItem}>{phaseLabel(t, spec)}</span>
        {spec.pendingQuestions > 0 && (
          <span className={styles.footAlert}>
            <Icon name="help" size={13} />
            {t('specs.pendingQuestions', { count: spec.pendingQuestions })}
          </span>
        )}
        {spec.openFindings > 0 && (
          <span className={styles.footAlert}>
            <Icon name="warning" size={13} />
            {t('specs.openFindings', { count: spec.openFindings })}
          </span>
        )}
        {spec.implementations > 0 && (
          <span className={styles.footItem}>
            <Icon name="workflow" size={13} />
            {t('specs.implementedBy', { count: spec.implementations })}
          </span>
        )}
        <span className={styles.footTime}>{agoText(spec.updatedAt, language, now)}</span>
      </span>
    </button>
  );
}
