import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { agoText } from '@/features/workflow/model/time';
import type { TranslationKey } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import { errorDetail } from '@/i18n/messages';
import type { CliStatusDto } from '@/lib/tauri/commands';
import { getCliStatus } from '../services/specService';
import type { SpecView } from '../types';
import styles from './Specs.module.css';

const REASONS: Record<string, TranslationKey> = {
  not_installed: 'specs.cli.reason.not_installed',
  inside_project: 'specs.cli.reason.inside_project',
  version_unreadable: 'specs.cli.reason.version_unreadable',
  too_old: 'specs.cli.reason.too_old',
};

interface Props {
  view: SpecView;
  busy: boolean;
  onCheck: () => void;
  onArchive: () => void;
  /** What the last attempt at either said, when it failed. */
  error: unknown;
}

/**
 * The OpenSpec command, as an extra. Atlas needs none of it to write, check, publish or import;
 * if it is installed it can give a second opinion on the published change and archive it. Both are
 * asked for here, one by one, and archiving says what it will do before it does it.
 */
export function CliPanel({ view, busy, onCheck, onArchive, error }: Props) {
  const { t, language } = useI18n();
  const [status, setStatus] = useState<CliStatusDto | null>(null);
  const [confirming, setConfirming] = useState(false);
  const published = view.spec.publication.state === 'published' && view.publicationCurrent;
  const specId = view.spec.id;

  useEffect(() => {
    if (!published) return;
    let current = true;
    Promise.resolve(getCliStatus(specId))
      .then((found) => {
        if (current) setStatus(found);
      })
      .catch(() => {
        if (current) setStatus(null);
      });
    return () => {
      current = false;
    };
  }, [specId, published]);

  if (!published || view.spec.archived) return null;
  const usable = status?.compatible === true;
  const check = view.spec.cliCheck;
  const detail = errorDetail(error);

  return (
    <section className={styles.panel} aria-label={t('specs.cli.title')}>
      <header className={styles.panelHead}>
        <h3>{t('specs.cli.title')}</h3>
        <span className={styles.panelTag} data-tone={usable ? 'success' : 'neutral'}>
          {status === null
            ? '…'
            : usable
              ? t('specs.cli.version', { version: status.version ?? '' })
              : t('specs.cli.unavailable')}
        </span>
      </header>
      <p className={styles.muted}>{t('specs.cli.intro')}</p>
      {status !== null && !usable && (
        <p className={styles.muted}>
          {t(REASONS[status.reason ?? ''] ?? 'specs.cli.reason.not_installed')}
        </p>
      )}
      {check && (
        <div role="status">
          <p className={check.valid ? styles.okLine : styles.errorLine}>
            {check.valid
              ? t('specs.cli.valid', { version: check.cliVersion })
              : t('specs.cli.invalid', { version: check.cliVersion })}{' '}
            <span className={styles.muted}>{agoText(check.at, language)}</span>
          </p>
          {check.issues.length > 0 && (
            <ul className={styles.plain}>
              {check.issues.map((issue, index) => (
                <li key={`${issue.path}-${String(index)}`}>
                  <span>{issue.message}</span>
                  <span className={styles.mono}>{issue.path}</span>
                </li>
              ))}
            </ul>
          )}
          <p className={styles.muted}>{t('specs.cli.secondOpinion')}</p>
        </div>
      )}
      {detail && (
        <details>
          <summary>{t('specs.cli.said')}</summary>
          <pre className={styles.source}>{detail}</pre>
        </details>
      )}
      <div className={styles.row}>
        <Button variant="secondary" disabled={!usable || busy} onClick={onCheck}>
          {t('specs.cli.check')}
        </Button>
        <Button
          variant="secondary"
          disabled={!usable || busy || !view.readiness.ready}
          onClick={() => {
            setConfirming(true);
          }}
        >
          {t('specs.cli.archive')}
        </Button>
      </div>
      {confirming && (
        <Modal
          label={t('specs.cli.archiveTitle')}
          onClose={() => {
            setConfirming(false);
          }}
        >
          <div className={styles.form}>
            <h2 className={styles.dialogTitle}>{t('specs.cli.archiveTitle')}</h2>
            <p>{t('specs.cli.archiveBody', { change: view.spec.changeId })}</p>
            <ul className={styles.plain}>
              <li>{t('specs.cli.archiveMerges')}</li>
              <li>{t('specs.cli.archiveMoves', { change: view.spec.changeId })}</li>
              <li>{t('specs.cli.archiveKeeps')}</li>
            </ul>
            <div className={styles.dialogActions}>
              <Button
                variant="secondary"
                onClick={() => {
                  setConfirming(false);
                }}
              >
                {t('common.cancel')}
              </Button>
              <Button
                disabled={busy}
                onClick={() => {
                  setConfirming(false);
                  onArchive();
                }}
              >
                {t('specs.cli.archiveConfirm')}
              </Button>
            </div>
          </div>
        </Modal>
      )}
    </section>
  );
}
