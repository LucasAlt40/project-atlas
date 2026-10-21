import { useState } from 'react';
import { Badge } from '@/components/ui/Badge';
import { Icon, type IconName } from '@/components/ui/Icon';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type {
  ImportDecisionDto,
  ImportItemPreviewDto,
  ImportPreviewDto,
  PortableKindDto,
} from '@/lib/tauri/commands';
import { decisionOf, without, type Choices } from '../model/importPlan';
import { issueText } from '../model/messages';
import { ModelPanel } from './ModelPanel';
import styles from './Portability.module.css';

export const KIND_ICON: Record<PortableKindDto, IconName> = {
  personality: 'personalities',
  agent: 'agents',
  workflow: 'workflow',
  workflow_template: 'layout',
};

interface Props {
  item: ImportItemPreviewDto;
  preview: ImportPreviewDto;
  choices: Choices;
  onChange: (change: (current: Choices) => Choices) => void;
}

function useFactValue() {
  const { t } = useI18n();
  return (label: string, value: string): string => {
    switch (label) {
      case 'permissions':
        return t(`security.profile.${value}` as TranslationKey);
      case 'isolation':
        return t(value === 'on' ? 'portability.value.on' : 'portability.value.off');
      case 'mode':
        return t(`portability.value.${value}` as TranslationKey);
      case 'contract':
        return value === '' ? t('agents.card.noResult') : value;
      default:
        return value;
    }
  };
}

/** One configuration of the file: what it is, what it matches here, what is wrong with it, and
 * what to do with it. Nothing is hidden: the instructions an agent would be given can be read. */
export function ImportItemCard({ item, preview, choices, onChange }: Props) {
  const { t } = useI18n();
  const factValue = useFactValue();
  const decision = decisionOf(item, choices);
  const writes = decision === 'create' || decision === 'replace';
  const nameOf = (ref: string) =>
    preview.items.find((i) => i.ref === ref)?.name ??
    preview.external.find((e) => e.ref === ref)?.name ??
    ref;
  const idBase = `import-${item.ref}`;
  // What was chosen, and what needs a look, is open; a dependency with nothing wrong waits folded.
  const needsLook =
    item.root || (item.existing !== null && !item.identical) || item.issues.length > 0;
  const [open, setOpen] = useState(needsLook);

  return (
    <li className={styles.card} data-skipped={decision === 'skip'} aria-label={item.name}>
      <header className={styles.cardHead}>
        <Badge mono>{t(`portability.kind.${item.kind}`)}</Badge>
        <h4 className={styles.name}>{item.name}</h4>
        <Badge mono tone={item.root ? 'accent' : 'neutral'}>
          {t(item.root ? 'portability.item.chosen' : 'portability.item.needed')}
        </Badge>
        {item.existing && (
          <Badge dot mono tone={item.identical ? 'success' : 'warning'}>
            {t(item.identical ? 'portability.conflict.identical' : 'portability.conflict.name', {
              name: item.existing.name,
            })}
          </Badge>
        )}
        <button
          type="button"
          className={styles.cardToggle}
          aria-expanded={open}
          aria-label={t(open ? 'agent.collapse' : 'agent.expand', { name: item.name })}
          onClick={() => {
            setOpen((current) => !current);
          }}
        >
          <Icon name={open ? 'chevronUp' : 'chevronDown'} size={16} />
        </button>
      </header>
      {open && (
        <div className={styles.cardBody}>
          {item.description && <p className={styles.muted}>{item.description}</p>}

          {item.facts.length > 0 && (
            <dl className={styles.facts}>
              {item.facts.map((fact) => (
                <div key={fact.label}>
                  <dt>{t(`portability.fact.${fact.label}` as TranslationKey)}</dt>
                  <dd>{factValue(fact.label, fact.value)}</dd>
                </div>
              ))}
            </dl>
          )}

          {item.dependsOn.length > 0 && (
            <p className={styles.muted}>
              {t('portability.item.dependsOn', { names: item.dependsOn.map(nameOf).join(', ') })}
            </p>
          )}

          {item.issues.length > 0 && (
            <ul
              className={styles.issues}
              aria-label={t('portability.item.issues', { name: item.name })}
            >
              {item.issues.map((issue) => (
                <li
                  key={`${issue.code}-${JSON.stringify(issue.params)}`}
                  data-severity={issue.severity}
                >
                  {issueText(t, issue)}
                </li>
              ))}
            </ul>
          )}

          <div className={styles.decision}>
            <label className={styles.field} htmlFor={`${idBase}-decision`}>
              {t('portability.decision.label', { name: item.name })}
            </label>
            <select
              id={`${idBase}-decision`}
              className={styles.control}
              value={decision}
              onChange={(e) => {
                const next = e.target.value as ImportDecisionDto;
                onChange((current) => ({
                  ...current,
                  decisions: { ...current.decisions, [item.ref]: next },
                  confirmedReplacements: current.confirmedReplacements.filter(
                    (r) => r !== item.ref,
                  ),
                }));
              }}
            >
              {item.allowed.map((allowed) => (
                <option key={allowed} value={allowed}>
                  {t(`portability.decision.${allowed}` as TranslationKey)}
                </option>
              ))}
            </select>
            {decision === 'create' && item.existing && (
              <span className={styles.muted}>{t('portability.conflict.renamed')}</span>
            )}
          </div>

          {decision === 'replace' && item.existing && (
            <div
              className={styles.replaceWarn}
              role="group"
              aria-label={t('portability.replace.title')}
            >
              <p className={styles.muted}>
                {t('portability.replace.warn', { name: item.existing.name })}
              </p>
              <label className={styles.check}>
                <input
                  type="checkbox"
                  checked={choices.confirmedReplacements.includes(item.ref)}
                  onChange={(e) => {
                    onChange((current) => ({
                      ...current,
                      confirmedReplacements: e.target.checked
                        ? [...current.confirmedReplacements, item.ref]
                        : current.confirmedReplacements.filter((r) => r !== item.ref),
                    }));
                  }}
                />
                {t('portability.replace.confirm', { name: item.existing.name })}
              </label>
            </div>
          )}

          {writes && item.model && (
            <ModelPanel
              name={item.name}
              model={item.model}
              pick={choices.models[item.ref]}
              onPick={(pick) => {
                onChange((current) => {
                  const rest = without(current.models, item.ref);
                  return { ...current, models: pick ? { ...rest, [item.ref]: pick } : rest };
                });
              }}
            />
          )}

          {(item.text !== null || item.steps.length > 0) && (
            <details className={styles.details}>
              <summary>{t('portability.details', { name: item.name })}</summary>
              {item.text && (
                <>
                  <pre className={styles.pre}>{item.text.value}</pre>
                  {item.text.truncated && (
                    <p className={styles.muted}>{t('portability.truncated')}</p>
                  )}
                </>
              )}
              {item.steps.length > 0 && (
                <ol className={styles.steps}>
                  {item.steps.map((step, index) => (
                    // A step's label is not unique; its place in the graph is.
                    <li key={`${String(index)}-${step.label}`}>
                      <strong>{step.label}</strong>
                      {' · '}
                      {t(`portability.step.${step.kind}` as TranslationKey)}
                      {step.agent ? ` · ${nameOf(step.agent)}` : ''}
                      {step.readOnly ? ` · ${t('portability.step.readOnly')}` : ''}
                      {step.instructions && (
                        <pre className={styles.pre}>{step.instructions.value}</pre>
                      )}
                    </li>
                  ))}
                </ol>
              )}
            </details>
          )}
        </div>
      )}
    </li>
  );
}
