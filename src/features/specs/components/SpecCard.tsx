import { Badge } from '@/components/ui/Badge';
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

/** One spec in the list, on one row: where it is, how far, and whether it waits for the person. */
export function SpecCard({ spec, agentName, now, onOpen }: Props) {
  const { t, language } = useI18n();
  const status = statusView(spec.status);
  return (
    <button
      type="button"
      className={styles.specRow}
      data-status={spec.status}
      onClick={() => {
        onOpen(spec.id);
      }}
      aria-label={spec.title}
    >
      <span className={styles.specMain}>
        <span className={styles.specTitleLine}>
          <span className={styles.specTitle}>{spec.title}</span>
          <span className={styles.idChip}>{spec.changeId}</span>
          <StatusBadge tone={status.tone}>{t(status.key)}</StatusBadge>
          {spec.pendingQuestions > 0 && (
            <Badge tone="warning">
              <Icon name="help" size={12} />
              {t('specs.pendingQuestions', { count: spec.pendingQuestions })}
            </Badge>
          )}
          {spec.openFindings > 0 && (
            <Badge tone="warning">
              <Icon name="warning" size={12} />
              {t('specs.openFindings', { count: spec.openFindings })}
            </Badge>
          )}
        </span>
        <span className={styles.specSub}>
          <span className={styles.specRequest}>{spec.request}</span>
          <span className={styles.specMeta}>
            <span>{agentName}</span>
            <span>{phaseLabel(t, spec)}</span>
            {spec.implementations > 0 && (
              <span>{t('specs.implementedBy', { count: spec.implementations })}</span>
            )}
          </span>
        </span>
      </span>
      <span className={styles.specProgress} data-tone={status.tone}>
        <span className={styles.progressText}>
          {t('specs.approvedOf', { approved: spec.approved, total: spec.stageTotal })}
        </span>
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
      </span>
      <span className={styles.specTime}>{agoText(spec.updatedAt, language, now)}</span>
    </button>
  );
}
