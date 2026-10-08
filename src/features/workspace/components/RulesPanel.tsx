import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type { ManifestRuleDto, RuleScopeDto } from '@/lib/tauri/commands';
import styles from './ContextReview.module.css';

const SCOPES: readonly RuleScopeDto[] = [
  'global',
  'project',
  'workspace',
  'workflow',
  'agent',
  'task',
];

/**
 * The rules that applied to one execution, by scope: whether each reached the agent (and if not,
 * why), how binding it was, where it came from and what its text may do. A rule is guidance, never
 * a permission, and the panel says so; a conflict or a rule kept as background is flagged where it is.
 */
export function RulesPanel({ rules }: { rules: ManifestRuleDto[] }) {
  const { t } = useI18n();

  return (
    <section className={styles.panel} aria-label={t('context.rules.title')}>
      <h3>{t('context.rules.title').toUpperCase()}</h3>
      <p className={styles.line}>{t('context.rules.note')}</p>
      {rules.some((rule) => rule.strength === 'mandatory' && rule.status === 'applied') && (
        <p className={styles.line}>{t('context.rules.checked')}</p>
      )}
      {rules.length === 0 && <p className={styles.line}>{t('context.rules.none')}</p>}
      {SCOPES.map((scope) => {
        const inScope = rules.filter((rule) => rule.scope === scope);
        if (inScope.length === 0) return null;
        return (
          <div key={scope}>
            <h4 className={styles.heading}>
              {t(`context.rules.scope.${scope}` as TranslationKey)}
            </h4>
            <ul className={styles.sources}>
              {inScope.map((rule) => {
                const applied = rule.status === 'applied';
                return (
                  <li key={rule.reference} data-status={rule.status}>
                    <span aria-hidden="true">{applied ? '✓' : '✗'}</span>{' '}
                    <strong>{rule.title}</strong>{' '}
                    <span className={styles.trust}>
                      ({t(`context.rules.strength.${rule.strength}` as TranslationKey)} ·{' '}
                      {t(`contextReview.authority.${rule.authority}` as TranslationKey)} ·{' '}
                      {t(`context.rules.origin.${rule.origin}` as TranslationKey)}{' '}
                      <code className={styles.mono}>{rule.source}</code>)
                    </span>{' '}
                    {!applied && (
                      <span className={styles.trust}>
                        {t(`context.rules.status.${rule.status}` as TranslationKey)}
                        {rule.by && ` ${t('context.rules.by', { by: rule.by })}`}
                      </span>
                    )}
                    {rule.inConflict && (
                      <span className={styles.severity} data-severity="warning">
                        {' '}
                        ⚠ {t('context.rules.conflict')}
                      </span>
                    )}
                    {rule.downgraded && (
                      <span className={styles.severity} data-severity="warning">
                        {' '}
                        ⚠ {t('context.rules.downgraded')}
                      </span>
                    )}
                  </li>
                );
              })}
            </ul>
          </div>
        );
      })}
    </section>
  );
}
