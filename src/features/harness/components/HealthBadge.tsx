import { useT } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type { HarnessHealthDto } from '@/lib/tauri/commands';
import styles from './Harness.module.css';

/** How far the Harness can be trusted, and why. */
export function HealthBadge({ health }: { health: HarnessHealthDto }) {
  const t = useT();
  const reasons = health.reasons
    .map((reason) => t(`harness.health.reason.${reason}` as TranslationKey))
    .join(' · ');
  return (
    <span
      className={styles.health}
      data-health={health.state}
      title={reasons || undefined}
      aria-label={`${t('harness.health.label')}: ${t(`harness.health.${health.state}`)}${reasons ? `. ${reasons}` : ''}`}
    >
      {t(`harness.health.${health.state}`)}
    </span>
  );
}
