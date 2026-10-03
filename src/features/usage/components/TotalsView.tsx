import type { UsageTotalsDto } from '@/lib/tauri/commands';
import { useI18n } from '@/i18n/I18nProvider';
import { formatCost, formatTokens } from '../model/format';

/**
 * A sum of what Atlas observed. A value the runtime never reported is shown as such (not as
 * zero), and a partial sum says how many runs it covers.
 */
export function TotalsView({
  totals,
  showTokens = true,
  stacked = false,
}: {
  totals: UsageTotalsDto;
  showTokens?: boolean;
  /** Cost above tokens, one per line, instead of one line separated by dots. */
  stacked?: boolean;
}) {
  const { t, language } = useI18n();
  const partial = (reported: number) =>
    reported > 0 && reported < totals.runs
      ? ` (${t('details.partial', { reported, runs: totals.runs })})`
      : '';
  return (
    <>
      <span data-metric="cost">
        {totals.cost !== null
          ? `${formatCost(totals.cost, totals.currency, language)}${partial(totals.runsWithCost)}`
          : t('details.notReported')}
      </span>
      {showTokens && (
        <>
          {stacked ? null : ' · '}
          <span data-metric="tokens">
            {totals.totalTokens !== null
              ? `${formatTokens(totals.totalTokens, language)} ${t('details.tokens').toLowerCase()}${partial(totals.runsWithTokens)}`
              : `${t('details.tokens')}: ${t('details.notReported').toLowerCase()}`}
          </span>
        </>
      )}
    </>
  );
}
