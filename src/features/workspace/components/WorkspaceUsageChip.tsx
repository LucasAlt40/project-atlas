import { useState } from 'react';
import { Modal } from '@/components/ui/Modal';
import { TotalsView } from '@/features/usage/components/TotalsView';
import { useWorkspaceUsage } from '@/features/usage/hooks/useUsage';
import { useT } from '@/i18n/I18nProvider';
import styles from './Workspace.module.css';

interface Props {
  workspaceId: string;
  /** Changes when an execution finishes, so the numbers are read again. */
  usageVersion: number;
}

/** A lightweight summary of what this workspace's agents consumed, as Atlas observed it. */
export function WorkspaceUsageChip({ workspaceId, usageVersion }: Props) {
  const t = useT();
  const usage = useWorkspaceUsage(workspaceId, usageVersion);
  const [open, setOpen] = useState(false);
  if (usage.status !== 'ready') return null;
  const { today, week, month } = usage.summary;

  return (
    <>
      <button
        type="button"
        className={styles.usageChip}
        aria-label={t('usage.title')}
        onClick={() => {
          setOpen(true);
        }}
      >
        <span className={styles.muted}>{t('usage.thisWorkspace')}</span>
        <span>
          {t('usage.today')}: <TotalsView totals={today} showTokens={false} />
        </span>
        <span>
          {t('usage.week')}: <TotalsView totals={week} showTokens={false} />
        </span>
      </button>
      {open && (
        <Modal
          label={t('usage.title')}
          onClose={() => {
            setOpen(false);
          }}
        >
          <h2 className={styles.panelTitle}>{t('usage.title')}</h2>
          <dl className={styles.usageList}>
            {(
              [
                ['usage.today', today],
                ['usage.week', week],
                ['usage.month', month],
              ] as const
            ).map(([label, totals]) => (
              <div key={label}>
                <dt>{t(label)}</dt>
                <dd>
                  <TotalsView totals={totals} />
                  <span className={styles.muted}>
                    {' '}
                    · {t('details.runs', { runs: totals.runs })}
                  </span>
                </dd>
              </div>
            ))}
          </dl>
          <p className={styles.muted}>
            <strong>{t('details.atlasTracked')}.</strong> {t('details.atlasTrackedNote')}{' '}
            {t('usage.quotaPerAgent')}
          </p>
        </Modal>
      )}
    </>
  );
}
