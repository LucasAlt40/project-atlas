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
export function HealthBadge({
  health,
  plain = false,
}: {
  health: HarnessHealthDto;
  /** Words only, in the tone's color (inside a line of text). */
  plain?: boolean;
}) {
  const t = useT();
  const reasons = health.reasons
    .map((reason) => t(`harness.health.reason.${reason}` as TranslationKey))
    .join(' · ');
  return (
    <span
      aria-label={`${t('harness.health.label')}: ${t(`harness.health.${health.state}`)}${reasons ? `. ${reasons}` : ''}`}
    >
      {plain ? (
        <span
          data-tone={TONE[health.state]}
          style={{
            color: health.state === 'healthy' ? 'inherit' : `var(--${TONE[health.state]})`,
          }}
          {...(reasons ? { title: reasons } : {})}
        >
          {t(`harness.health.${health.state}`)}
        </span>
      ) : (
        <Badge dot tone={TONE[health.state]} {...(reasons ? { title: reasons } : {})}>
          {t(`harness.health.${health.state}`)}
        </Badge>
      )}
    </span>
  );
}
