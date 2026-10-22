import { useCallback, useEffect, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Callout } from '@/components/ui/Callout';
import { Modal } from '@/components/ui/Modal';
import type { TranslationKey } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import type {
  CleanupChoiceDto,
  CleanupReportDto,
  StoragePartKindDto,
  StorageUsageDto,
} from '@/lib/tauri/commands';
import { cleanStorage, getStorageUsage } from '../services/settingsService';
import styles from './StorageSettings.module.css';

const UNITS = ['byte', 'kilobyte', 'megabyte', 'gigabyte', 'terabyte'] as const;

/** `42 MB`, in the interface's language (powers of 1000, as the system's file manager). */
export function formatBytes(bytes: number, language: string): string {
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  return new Intl.NumberFormat(language, {
    style: 'unit',
    unit: UNITS[unit],
    unitDisplay: 'short',
    maximumFractionDigits: unit === 0 || value >= 100 ? 0 : 1,
  }).format(value);
}

type Choice = keyof CleanupChoiceDto;
const CHOICES: readonly Choice[] = ['worktrees', 'sandboxTemp', 'branches'];

/**
 * How much disk Atlas uses and the cleanup of what executions leave behind: worktree folders,
 * the sandbox's temporary files and, only when asked, the `atlas/exec-…` branches.
 */
export function StorageSettings() {
  const { t, language } = useI18n();
  const [usage, setUsage] = useState<StorageUsageDto | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [choice, setChoice] = useState<CleanupChoiceDto>({
    worktrees: true,
    sandboxTemp: true,
    branches: false,
  });
  const [confirming, setConfirming] = useState(false);
  const [cleaning, setCleaning] = useState(false);
  const [report, setReport] = useState<CleanupReportDto | null>(null);

  const load = useCallback(() => {
    return Promise.resolve(getStorageUsage())
      .then((loaded) => {
        setUsage(loaded);
      })
      .catch((error: unknown) => {
        setFailure(errorMessage(t, error));
      });
  }, [t]);

  useEffect(() => {
    void load();
  }, [load]);

  async function clean() {
    setCleaning(true);
    setFailure(null);
    try {
      setReport(await cleanStorage(choice));
    } catch (error) {
      setFailure(errorMessage(t, error));
    } finally {
      setCleaning(false);
      setConfirming(false);
      await load();
    }
  }

  if (!usage) {
    return failure ? (
      <Callout tone="danger" role="alert">
        {failure}
      </Callout>
    ) : (
      <p className={styles.muted}>{t('settings.storage.measuring')}</p>
    );
  }

  const size = (bytes: number) => formatBytes(bytes, language);
  /** `key` for many, `key.one` for one. */
  const count = (key: string, n: number, extra: Record<string, string> = {}) =>
    t((n === 1 ? `${key}.one` : key) as TranslationKey, { count: n, ...extra });
  const { leftovers } = usage;
  const available: Record<Choice, boolean> = {
    worktrees: leftovers.worktrees.count > 0,
    sandboxTemp: leftovers.sandboxTemp.count > 0,
    branches: leftovers.branches > 0,
  };
  const amount: Record<Choice, string> = {
    worktrees: count('settings.storage.folders', leftovers.worktrees.count, {
      size: size(leftovers.worktrees.bytes),
    }),
    sandboxTemp: count('settings.storage.items', leftovers.sandboxTemp.count, {
      size: size(leftovers.sandboxTemp.bytes),
    }),
    branches: count('settings.storage.branchCount', leftovers.branches),
  };
  const chosen = CHOICES.filter((key) => choice[key] && available[key]);
  const freeable =
    (choice.worktrees ? leftovers.worktrees.bytes : 0) +
    (choice.sandboxTemp ? leftovers.sandboxTemp.bytes : 0);

  return (
    <div className={styles.root}>
      <div className={styles.group}>
        <div className={styles.headRow}>
          <h3 className={styles.heading}>{t('settings.storage.usage')}</h3>
          <Button
            variant="ghost"
            onClick={() => {
              setFailure(null);
              setReport(null);
              void load();
            }}
          >
            {t('settings.storage.refresh')}
          </Button>
        </div>
        <p className={styles.total}>
          <span className={styles.totalValue}>{size(usage.totalBytes)}</span>
          <span className={styles.muted}>{t('settings.storage.totalHelp')}</span>
        </p>
        <p className={styles.path} title={usage.folder}>
          {usage.folder}
        </p>
        {usage.totalBytes > 0 && (
          <>
            <div className={styles.bar} aria-hidden="true">
              {usage.parts.map((part) => (
                <span
                  key={part.kind}
                  className={styles.segment}
                  data-kind={part.kind}
                  style={{ flexGrow: part.bytes }}
                />
              ))}
            </div>
            <ul className={styles.legend} aria-label={t('settings.storage.usage')}>
              {usage.parts.map((part) => (
                <li key={part.kind} className={styles.legendRow}>
                  <span className={styles.dot} data-kind={part.kind} aria-hidden="true" />
                  <span className={styles.legendLabel}>
                    {t(`settings.storage.part.${part.kind satisfies StoragePartKindDto}`)}
                  </span>
                  <span className={styles.legendSize}>{size(part.bytes)}</span>
                </li>
              ))}
            </ul>
          </>
        )}
      </div>

      <div className={styles.group}>
        <h3 className={styles.heading}>{t('settings.storage.cleanTitle')}</h3>
        <p className={styles.muted}>{t('settings.storage.cleanHelp')}</p>
        {usage.running && (
          <Callout tone="warning" role="status">
            {t('error.storage_busy')}
          </Callout>
        )}
        <ul className={styles.options}>
          {CHOICES.map((key) => (
            <li key={key} className={styles.option} data-danger={key === 'branches' || undefined}>
              <label className={styles.optionLabel}>
                <input
                  type="checkbox"
                  checked={choice[key] && available[key]}
                  disabled={!available[key]}
                  onChange={(event) => {
                    setChoice((current) => ({ ...current, [key]: event.target.checked }));
                  }}
                />
                <span className={styles.optionText}>
                  <span className={styles.optionTitle}>{t(`settings.storage.${key}`)}</span>
                  <span className={styles.muted}>{t(`settings.storage.${key}Help`)}</span>
                </span>
                <span className={styles.optionAmount}>
                  {available[key] ? amount[key] : t('settings.storage.nothing')}
                </span>
              </label>
            </li>
          ))}
        </ul>
        <div className={styles.actions}>
          <Button
            variant={choice.branches && available.branches ? 'danger' : 'primary'}
            disabled={chosen.length === 0 || usage.running || cleaning}
            onClick={() => {
              setReport(null);
              setConfirming(true);
            }}
          >
            {t('settings.storage.cleanButton')}
          </Button>
          {freeable > 0 && (
            <span className={styles.muted}>
              {t('settings.storage.frees', { size: size(freeable) })}
            </span>
          )}
        </div>
        {report && (
          <Callout tone="success" role="status" title={t('settings.storage.doneTitle')}>
            {[
              t('settings.storage.done', { size: size(report.freedBytes) }),
              report.worktreesRemoved > 0 &&
                count('settings.storage.doneWorktrees', report.worktreesRemoved),
              report.sandboxTempRemoved > 0 &&
                count('settings.storage.doneTemp', report.sandboxTempRemoved),
              report.branchesRemoved > 0 &&
                count('settings.storage.doneBranches', report.branchesRemoved),
              report.worktreesKept > 0 &&
                count('settings.storage.keptWorktrees', report.worktreesKept),
              report.branchesKept > 0 &&
                count('settings.storage.keptBranches', report.branchesKept),
            ]
              .filter(Boolean)
              .join(' ')}
          </Callout>
        )}
        {failure && (
          <Callout tone="danger" role="alert">
            {failure}
          </Callout>
        )}
      </div>

      {confirming && (
        <Modal
          label={t('settings.storage.confirmTitle')}
          title={t('settings.storage.confirmTitle')}
          size="sm"
          onClose={() => {
            if (!cleaning) setConfirming(false);
          }}
          footer={
            <>
              <Button
                variant="secondary"
                disabled={cleaning}
                onClick={() => {
                  setConfirming(false);
                }}
              >
                {t('common.cancel')}
              </Button>
              <Button
                variant={choice.branches && available.branches ? 'danger' : 'primary'}
                disabled={cleaning}
                onClick={() => {
                  void clean();
                }}
              >
                {t(cleaning ? 'settings.storage.cleaning' : 'settings.storage.confirm')}
              </Button>
            </>
          }
        >
          <ul className={styles.confirmList}>
            {chosen.map((key) => (
              <li key={key}>
                <strong>{t(`settings.storage.${key}`)}</strong> · {amount[key]}
              </li>
            ))}
          </ul>
          <p className={styles.muted}>{t('settings.storage.confirmSafe')}</p>
          {choice.branches && available.branches && (
            <Callout tone="danger">{t('settings.storage.confirmBranches')}</Callout>
          )}
        </Modal>
      )}
    </div>
  );
}
