import { useI18n } from '@/i18n/I18nProvider';
import type { KnowledgeStatsDto } from '@/lib/tauri/commands';
import { formatAnalyzed } from './StaleNotice';
import styles from './Harness.module.css';

/** What the Harness holds, at a glance: how much is verified, inferred, from the user, unknown. */
export function HarnessStats({
  stats,
  analyzedAt,
}: {
  stats: KnowledgeStatsDto;
  analyzedAt: number | null;
}) {
  const { t, language } = useI18n();
  const parts = [
    t('harness.stats.findings', { count: stats.findings }),
    t('harness.stats.verified', { count: stats.verified }),
    t('harness.stats.inferred', { count: stats.inferred }),
    stats.user > 0 ? t('harness.stats.user', { count: stats.user }) : null,
    stats.stale > 0 ? t('harness.stats.stale', { count: stats.stale }) : null,
    t('harness.stats.unknown', { count: stats.unknown }),
  ].filter((part): part is string => part !== null);
  return (
    <span className={styles.muted}>
      {parts.join(' · ')}
      {analyzedAt
        ? ` — ${t('harness.stats.lastAnalyzed', { date: formatAnalyzed(analyzedAt, language) })}`
        : ''}
    </span>
  );
}
