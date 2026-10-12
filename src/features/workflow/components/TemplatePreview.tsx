import { Button } from '@/components/ui/Button';
import { useI18n } from '@/i18n/I18nProvider';
import type { Translate, TranslationKey } from '@/i18n';
import type { ToolPlanItemDto } from '@/lib/tauri/commands';
import { canGrantTools, goalKey, outputKey, REASON_KEY } from '../model/templateView';
import type { WorkflowTemplate } from '../types';
import styles from './Workflow.module.css';

interface Props {
  template: WorkflowTemplate;
  /** The template's suggested tools resolved against this workspace; `null` while it loads. */
  plan: ToolPlanItemDto[] | null;
  /** The name of an agent a saved template's step uses. */
  agentName: (agentId: string) => string;
  applyTools: boolean;
  onApplyTools: (apply: boolean) => void;
  /** Only a template the person saved can be deleted. */
  onDelete?: () => void;
}

function toolLine(t: Translate, item: ToolPlanItemDto): string {
  const kind = item.capability
    ? t(`workflow.template.toolKind.${item.capability}` as TranslationKey)
    : (item.connection ?? '');
  if (item.reason) {
    const reason = t(REASON_KEY[item.reason.kind], {
      name: item.reason.kind === 'needs_secret' ? item.reason.name : '',
    });
    return t('workflow.template.toolMissing', { kind, reason });
  }
  const reach =
    item.tools?.kind === 'only'
      ? t('workflow.template.toolOnly', { tools: item.tools.tools.join(', ') })
      : t('workflow.template.toolAll');
  return `${t('workflow.template.toolReady', { kind, connection: item.connection ?? '' })} (${reach})`;
}

/**
 * What a template will do, before anything runs: its goal, each step with who does it and when it
 * is done, which tools each step may get here and why not when it cannot, and what comes out.
 */
export function TemplatePreview({
  template,
  plan,
  agentName,
  applyTools,
  onApplyTools,
  onDelete,
}: Props) {
  const { t } = useI18n();
  const goal = goalKey(template);
  const stepLabel = (id: string) => template.steps.find((s) => s.id === id)?.label ?? id;
  const custom = template.kind === 'custom';
  const tools = plan ?? [];

  return (
    <section className={styles.preview} aria-label={t('workflow.template.details')}>
      <header className={styles.previewHeader}>
        <h3>{template.name}</h3>
        <span className={styles.tag} data-kind={template.kind}>
          {t(`workflow.template.badge.${template.kind}` as TranslationKey)}
        </span>
      </header>
      <p>{goal ? t(goal) : template.goal || template.description}</p>
      {template.readOnly && (
        <p className={styles.recommendation}>{t('workflow.template.analysisOnly')}</p>
      )}

      <h4>{t('workflow.template.steps')}</h4>
      <ol className={styles.previewSteps}>
        {template.steps.map((step) => (
          <li key={step.id}>
            <details>
              <summary>
                <strong>{step.label}</strong>
                <span className={styles.muted}>
                  {' · '}
                  {step.role
                    ? t(`workflow.role.${step.role}` as TranslationKey)
                    : step.agentId
                      ? agentName(step.agentId)
                      : ''}
                </span>
                {step.readOnly && (
                  <span className={styles.tag} data-kind="read-only">
                    {t('workflow.template.step.readOnly')}
                  </span>
                )}
                {step.maxIterations !== null && (
                  <span className={styles.muted}>
                    {' · '}
                    {t('workflow.template.step.loop', { max: step.maxIterations })}
                  </span>
                )}
              </summary>
              {step.completionCriteria && (
                <p>
                  <strong>{t('workflow.template.step.doneWhen')}: </strong>
                  {step.completionCriteria}
                </p>
              )}
              {step.instructions && (
                <>
                  <p className={styles.fieldLabel}>{t('workflow.template.step.instructions')}</p>
                  <p className={styles.previewText}>{step.instructions}</p>
                </>
              )}
            </details>
          </li>
        ))}
      </ol>

      <h4>{t('workflow.template.tools')}</h4>
      {template.tools.length === 0 ? (
        <p className={styles.muted}>{t('workflow.template.toolsNone')}</p>
      ) : (
        <>
          <ul className={styles.previewTools}>
            {tools.map((item, index) => (
              <li key={`${item.nodeId}-${String(index)}`} data-available={item.reason === null}>
                <span>{toolLine(t, item)}</span>{' '}
                <span className={styles.muted}>
                  {t('workflow.template.toolStep', { step: stepLabel(item.nodeId) })}
                  {item.purpose ? ` — ${item.purpose}` : ''}
                </span>
              </li>
            ))}
          </ul>
          <p className={styles.muted}>{t('workflow.template.toolsNote')}</p>
          <label className={styles.checkRow}>
            <input
              type="checkbox"
              checked={applyTools && canGrantTools(tools)}
              disabled={!canGrantTools(tools)}
              onChange={(e) => {
                onApplyTools(e.target.checked);
              }}
            />{' '}
            {t('workflow.template.applyTools')}
          </label>
          <p className={styles.muted}>{t('workflow.template.applyToolsHint')}</p>
        </>
      )}

      {template.outputs.length > 0 && (
        <>
          <h4>{t('workflow.template.outputs')}</h4>
          <ul>
            {template.outputs.map((text, index) => {
              const key = outputKey(template, index);
              return <li key={text}>{key ? t(key) : text}</li>;
            })}
          </ul>
        </>
      )}

      <p className={styles.muted}>{t('workflow.template.scope')}</p>
      {custom && onDelete && (
        <div className={styles.actions}>
          <Button type="button" variant="danger" onClick={onDelete}>
            {t('workflow.template.delete')}
          </Button>
          <span className={styles.muted}>{t('workflow.template.deleteHint')}</span>
        </div>
      )}
    </section>
  );
}
