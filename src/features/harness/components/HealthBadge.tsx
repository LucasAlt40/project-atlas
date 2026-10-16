import { Badge, type BadgeTone } from '@/components/ui/Badge';
import { useT } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type { HarnessHealthDto } from '@/lib/tauri/commands';

const TONE: Record<HarnessHealthDto['state'], BadgeTone> = {
  healthy: 'success',
  conflicted: 'danger',
  needs_review: 'warning',
  partial: 'warning',
  stale: 'warning',
};

/** How far the Harness can be trusted, and why. */
export function HealthBadge({ health }: { health: HarnessHealthDto }) {
  const t = useT();
  const reasons = health.reasons
    .map((reason) => t(`harness.health.reason.${reason}` as TranslationKey))
    .join(' · ');
  return (
    <span
      aria-label={`${t('harness.health.label')}: ${t(`harness.health.${health.state}`)}${reasons ? `. ${reasons}` : ''}`}
    >
      <Badge dot tone={TONE[health.state]} {...(reasons ? { title: reasons } : {})}>
        {t(`harness.health.${health.state}`)}
      </Badge>
    </span>
  );
}
