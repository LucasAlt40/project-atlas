import { useT } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import { clock } from './format';
import type { Alert, AlertKind, Remote } from './useRemote';
import styles from './Mobile.module.css';

const ALERT_KEY: Record<AlertKind, TranslationKey> = {
  needs_answer: 'mobile.alert.needsAnswer',
  needs_computer: 'mobile.alert.needsComputer',
  failed: 'mobile.alert.failed',
  completed: 'mobile.alert.completed',
};

/** Says out loud when the computer cannot be reached, instead of showing stale data as if it were live. */
export function ConnectionBanner({ remote }: { remote: Remote }) {
  const t = useT();
  if (remote.link === 'live' || remote.link === 'connecting') return null;
  const since = remote.lastSyncAt
    ? t('mobile.link.lastUpdate', { time: clock(remote.lastSyncAt, remote.language ?? 'en-US') })
    : '';
  return (
    <div
      className={styles.banner}
      data-tone={remote.link === 'offline' ? 'bad' : 'attention'}
      role="status"
    >
      <strong>
        {remote.link === 'offline' ? t('mobile.link.offline') : t('mobile.link.reconnecting')}
      </strong>
      {remote.link === 'offline' && <span> {t('mobile.link.offlineHelp')}</span>}
      {since && <span> {since}</span>}
    </div>
  );
}

/** What changed while the page is open: a question waiting, a failure, a run that finished. */
export function AlertBanners({
  alerts,
  onOpen,
  onDismiss,
}: {
  alerts: Alert[];
  onOpen: (alert: Alert) => void;
  onDismiss: (id: string) => void;
}) {
  const t = useT();
  if (alerts.length === 0) return null;
  return (
    <div className={styles.alerts} role="alert">
      {alerts.map((alert) => (
        <div key={alert.id} className={styles.alert} data-kind={alert.kind}>
          <button
            type="button"
            className={styles.alertBody}
            onClick={() => {
              onOpen(alert);
            }}
          >
            <span className={styles.alertTitle}>{t(ALERT_KEY[alert.kind])}</span>
            <span className={styles.alertMeta}>{alert.title}</span>
          </button>
          <button
            type="button"
            className={styles.iconButton}
            aria-label={t('mobile.alert.dismiss')}
            onClick={() => {
              onDismiss(alert.id);
            }}
          >
            ✕
          </button>
        </div>
      ))}
    </div>
  );
}
