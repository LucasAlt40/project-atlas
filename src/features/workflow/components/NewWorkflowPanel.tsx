import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type { ToolPlanItemDto, WorkflowModeDto } from '@/lib/tauri/commands';
import type { WorkflowTemplate } from '../types';
import { canGrantTools } from '../model/templateView';
import { TemplatePreview } from './TemplatePreview';
import styles from './Workflow.module.css';

export interface NewWorkflowChoice {
  templateId: string | null;
  mode: WorkflowModeDto;
  name: string;
  task: string;
  /** Give each step the tools the template suggests for it (only what the workspace can give). */
  applyTools: boolean;
}

interface Props {
  templates: WorkflowTemplate[];
  /** The template Automatic mode proposes for a task. */
  recommend: (task: string) => Promise<string>;
  /** What a template's suggested tools come to in this workspace. */
  planTools: (templateId: string) => Promise<ToolPlanItemDto[]>;
  /** The name of an agent, for the steps of a template the person saved. */
  agentName: (agentId: string) => string;
  onDeleteTemplate: (templateId: string) => void;
  /** Why creating failed, if it did. */
  error: string | null;
  onCreate: (choice: NewWorkflowChoice) => void;
  onCancel?: () => void;
}

/**
 * Starting a workflow. Automatic: the user gives the task and Atlas proposes a recommended
 * workflow (chosen by fixed rules, not by a model). Custom: start from a template or from
 * nothing and compose it. Both make the same kind of workflow; only who laid out the graph differs.
 */
export function NewWorkflowPanel({
  templates,
  recommend,
  planTools,
  agentName,
  onDeleteTemplate,
  error,
  onCreate,
  onCancel,
}: Props) {
  const { t } = useI18n();
  const [mode, setMode] = useState<WorkflowModeDto>('automatic');
  const [task, setTask] = useState('');
  const [name, setName] = useState('');
  const [templateId, setTemplateId] = useState<string>('');
  const [recommended, setRecommended] = useState<string | null>(null);
  const [applyTools, setApplyTools] = useState(true);
  const [plan, setPlan] = useState<{ templateId: string; items: ToolPlanItemDto[] } | null>(null);

  useEffect(() => {
    if (mode !== 'automatic') return;
    let cancelled = false;
    const handle = setTimeout(() => {
      recommend(task)
        .then((id) => {
          if (!cancelled) setRecommended(id);
        })
        .catch(() => {
          if (!cancelled) setRecommended(null);
        });
    }, 150);
    return () => {
      cancelled = true;
      clearTimeout(handle);
    };
  }, [mode, task, recommend]);

  const proposal = templates.find((template) => template.id === recommended);
  const chosen = mode === 'automatic' ? (recommended ?? null) : templateId || null;
  const label = (template: WorkflowTemplate) =>
    template.kind === 'custom'
      ? template.name
      : t(`workflow.template.${template.id}` as TranslationKey);
  const shown = templates.find((template) => template.id === chosen);
  const builtin = templates.filter((template) => template.kind === 'builtin');
  const saved = templates.filter((template) => template.kind === 'custom');

  // The tools the chosen template would get here: read again when the template (or the list of
  // templates, after one is deleted) changes.
  useEffect(() => {
    if (!chosen) return;
    let cancelled = false;
    planTools(chosen)
      .then((items) => {
        if (!cancelled) setPlan({ templateId: chosen, items });
      })
      .catch(() => {
        if (!cancelled) setPlan({ templateId: chosen, items: [] });
      });
    return () => {
      cancelled = true;
    };
  }, [chosen, planTools]);

  return (
    <form
      className={styles.newPanel}
      aria-label={t('workflow.new.title')}
      onSubmit={(event) => {
        event.preventDefault();
        onCreate({
          templateId: chosen,
          mode,
          name: name.trim(),
          task: task.trim(),
          applyTools: applyTools && canGrantTools(plan?.templateId === chosen ? plan.items : []),
        });
      }}
    >
      <h2>{t('workflow.new.title')}</h2>
      <fieldset className={styles.modes}>
        <legend>{t('workflow.new.mode')}</legend>
        {(['automatic', 'custom'] as const).map((value) => (
          <label key={value} className={styles.mode}>
            <input
              type="radio"
              name="workflow-mode"
              value={value}
              checked={mode === value}
              onChange={() => {
                setMode(value);
              }}
            />
            <span>
              <strong>{t(`workflow.mode.${value}` as TranslationKey)}</strong>
              <span className={styles.muted}>
                {' '}
                — {t(`workflow.mode.${value}.help` as TranslationKey)}
              </span>
            </span>
          </label>
        ))}
      </fieldset>

      {mode === 'automatic' ? (
        <>
          <label className={styles.field}>
            <span className={styles.fieldLabel}>{t('workflow.new.task')}</span>
            <textarea
              className={styles.textarea}
              value={task}
              placeholder={t('workflow.new.taskPlaceholder')}
              onChange={(e) => {
                setTask(e.target.value);
              }}
            />
          </label>
          {proposal && (
            <p className={styles.recommendation} role="status">
              {t('workflow.new.recommended', { name: label(proposal) })}
              <span className={styles.muted}>
                {' '}
                — {t(`workflow.templateHelp.${proposal.id}` as TranslationKey)}
              </span>
            </p>
          )}
        </>
      ) : (
        <fieldset className={styles.templateGroup}>
          <legend className={styles.fieldLabel}>{t('workflow.new.template')}</legend>
          <div className={styles.templateGrid}>
            {[...builtin, ...saved].map((template) => (
              <label
                key={template.id}
                className={styles.templateCard}
                data-selected={templateId === template.id}
              >
                <input
                  type="radio"
                  name="workflow-template"
                  value={template.id}
                  checked={templateId === template.id}
                  onChange={() => {
                    setTemplateId(template.id);
                  }}
                />
                <span className={styles.templateName}>
                  {label(template)}
                  <span className={styles.tag} data-kind={template.kind}>
                    {t(`workflow.template.badge.${template.kind}` as TranslationKey)}
                  </span>
                </span>
                <span className={styles.muted}>
                  {template.kind === 'custom'
                    ? template.description
                    : t(`workflow.templateHelp.${template.id}` as TranslationKey)}
                </span>
                <span className={styles.templateMeta}>
                  {t('workflow.template.meta', {
                    steps: template.steps.length,
                    tools: template.tools.length,
                  })}
                  {template.readOnly && ` · ${t('workflow.template.step.readOnly')}`}
                </span>
              </label>
            ))}
            <label className={styles.templateCard} data-selected={templateId === ''}>
              <input
                type="radio"
                name="workflow-template"
                value=""
                checked={templateId === ''}
                onChange={() => {
                  setTemplateId('');
                }}
              />
              <span className={styles.templateName}>{t('workflow.new.blank')}</span>
            </label>
          </div>
        </fieldset>
      )}

      {shown && (
        <TemplatePreview
          template={shown}
          plan={plan?.templateId === shown.id ? plan.items : null}
          agentName={agentName}
          applyTools={applyTools}
          onApplyTools={setApplyTools}
          {...(shown.kind === 'custom'
            ? {
                onDelete: () => {
                  setTemplateId('');
                  onDeleteTemplate(shown.id);
                },
              }
            : {})}
        />
      )}

      <label className={styles.field}>
        <span className={styles.fieldLabel}>{t('workflow.name')}</span>
        <input
          className={styles.control}
          value={name}
          placeholder={t('workflow.new.namePlaceholder')}
          onChange={(e) => {
            setName(e.target.value);
          }}
        />
      </label>
      {error && (
        <p role="alert" className={styles.failure}>
          {error}
        </p>
      )}
      <div className={styles.actions}>
        {onCancel && (
          <Button type="button" variant="secondary" onClick={onCancel}>
            {t('common.cancel')}
          </Button>
        )}
        <Button type="submit" disabled={mode === 'automatic' && chosen === null}>
          {t('workflow.new.create')}
        </Button>
      </div>
    </form>
  );
}
