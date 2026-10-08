import { formatTokens } from '@/features/usage/model/format';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type {
  FigureDto,
  RuntimeSurfaceDto,
  StoredExecutionDto,
  SurfaceControlDto,
} from '@/lib/tauri/commands';
import styles from './ContextReview.module.css';
import { McpPanel } from './McpPanel';
import { RulesPanel } from './RulesPanel';

const CONTROLS: readonly SurfaceControlDto[] = [
  'atlas_controlled',
  'runtime_controlled',
  'user_controlled',
  'unknown',
];

/**
 * The context contract of one execution: what it may use (budget), what Atlas meant to send
 * (plan), what it prepared and delivered (manifest, with the hash of the payload) and what else
 * shapes the run and who controls it (surface). An estimate is always shown as one (`~`, with its
 * method); a number nobody stated is "unknown", never a guess. A reported or declared source is
 * not a claim that the model received it: only the delivered prompt is.
 */
export function ContextPlatformPanel({ execution }: { execution: StoredExecutionDto }) {
  const { t, language } = useI18n();
  const { plan, manifest } = execution;
  const budget = execution.optimization?.budget;

  const figure = (value: FigureDto) =>
    value.value === null
      ? t('context.unknown')
      : `${value.precision === 'estimated' ? '~' : ''}${formatTokens(value.value, language)}`;
  const provenance = (value: FigureDto) =>
    t('context.provenance', {
      source: t(`context.source.${value.source}` as TranslationKey),
      precision: t(`context.precision.${value.precision}` as TranslationKey),
      method: value.method ? ` · ${t(`context.method.${value.method}` as TranslationKey)}` : '',
    });
  const row = (label: string, value: FigureDto) => (
    <>
      <dt>{label}</dt>
      <dd>
        {figure(value)} <span className={styles.trust}>({provenance(value)})</span>
      </dd>
    </>
  );
  const section = (kind: string) => t(`contextReview.section.${kind}` as TranslationKey);
  const surface: RuntimeSurfaceDto | undefined = manifest?.surface;

  return (
    <div aria-label={t('inspector.tab.context')}>
      {budget && (
        <section className={styles.panel} aria-label={t('context.budget.title')}>
          <h3>{t('context.budget.title').toUpperCase()}</h3>
          <dl className={styles.detail}>
            <dt>{t('inspector.runtime')}</dt>
            <dd>{budget.runtimeId}</dd>
            <dt>{t('inspector.model')}</dt>
            <dd>{budget.modelId}</dd>
            {row(t('context.budget.inputLimit'), budget.input.limit)}
            {row(t('context.budget.inputUsed'), budget.input.used)}
            {row(t('context.budget.inputRemaining'), budget.input.remaining)}
            {row(t('context.budget.outputLimit'), budget.limits.output)}
            {row(t('context.budget.totalLimit'), budget.limits.total)}
          </dl>
        </section>
      )}
      {plan && (
        <section className={styles.panel} aria-label={t('context.plan.title')}>
          <h3>{t('context.plan.title').toUpperCase()}</h3>
          <ul className={styles.sources}>
            {plan.sections.map((part) => (
              <li key={part.section}>
                {section(part.section)}: {figure(part.tokens)}{' '}
                <span className={styles.trust}>
                  ({part.bytes.toLocaleString(language)} {t('context.bytes')})
                </span>
              </li>
            ))}
          </ul>
          <p className={styles.line}>
            {t('context.plan.total', { tokens: figure(plan.totalTokens) })}
            {plan.engineOmittedItems > 0 &&
              ` · ${t('context.plan.omitted', { count: plan.engineOmittedItems })}`}
          </p>
        </section>
      )}
      {manifest && (
        <section className={styles.panel} aria-label={t('context.manifest.title')}>
          <h3>{t('context.manifest.title').toUpperCase()}</h3>
          <dl className={styles.detail}>
            <dt>{t('context.manifest.delivered')}</dt>
            <dd>
              {manifest.delivery.delivered
                ? t('context.manifest.deliveredYes')
                : t('context.manifest.deliveredNo')}
            </dd>
            <dt>{t('context.manifest.size')}</dt>
            <dd>
              {manifest.delivery.bytes.toLocaleString(language)} {t('context.bytes')} ·{' '}
              {figure(manifest.delivery.estimatedTokens)}
            </dd>
            <dt>{t('context.manifest.channel')}</dt>
            <dd>
              {t(
                `context.manifest.channel.${manifest.delivery.systemChannel ?? 'unsupported'}` as TranslationKey,
              )}
              {(manifest.delivery.systemBytes ?? 0) > 0 &&
                ` (${(manifest.delivery.systemBytes ?? 0).toLocaleString(language)} ${t('context.bytes')})`}
            </dd>
            <dt>{t('context.manifest.hash')}</dt>
            <dd>
              <code className={styles.mono}>{manifest.delivery.promptHash}</code>
            </dd>
            <dt>{t('context.manifest.plan')}</dt>
            <dd>
              {manifest.divergedFromPlan
                ? t('context.manifest.planDiverged')
                : t('context.manifest.planSame')}
            </dd>
          </dl>
        </section>
      )}
      {manifest && <RulesPanel rules={manifest.rules ?? []} />}
      {manifest?.mcp && manifest.mcp.servers.length > 0 && <McpPanel record={manifest.mcp} />}
      {surface && (
        <section className={styles.panel} aria-label={t('context.surface.title')}>
          <h3>{t('context.surface.title').toUpperCase()}</h3>
          <p className={styles.line}>{t('context.surface.note')}</p>
          {CONTROLS.map((control) => {
            const entries = surface.entries.filter((entry) => entry.control === control);
            if (entries.length === 0) return null;
            return (
              <div key={control}>
                <h4 className={styles.heading}>
                  {t(`context.surface.control.${control}` as TranslationKey)}
                </h4>
                <ul className={styles.sources}>
                  {entries.map((entry, index) => (
                    <li key={`${entry.kind}-${String(index)}`}>
                      {t(`context.surface.kind.${entry.kind}` as TranslationKey)}{' '}
                      <span className={styles.trust}>
                        ({t(`context.surface.observation.${entry.observation}` as TranslationKey)}
                        {entry.reachesModel && ` · ${t('context.surface.reachesModel')}`})
                      </span>
                      {entry.detail && (
                        <>
                          {' '}
                          <code className={styles.mono}>
                            {entry.kind === 'system_channel'
                              ? t(`context.surface.channel.${entry.detail}` as TranslationKey)
                              : entry.detail}
                          </code>
                        </>
                      )}
                    </li>
                  ))}
                </ul>
              </div>
            );
          })}
        </section>
      )}
      {manifest && manifest.warnings.length > 0 && (
        <section className={styles.panel} aria-label={t('context.warnings.title')}>
          <h3>{t('context.warnings.title').toUpperCase()}</h3>
          <ul className={styles.issues}>
            {manifest.warnings.map((warning) => (
              <li key={warning}>{t(`context.warning.${warning}` as TranslationKey)}</li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}
