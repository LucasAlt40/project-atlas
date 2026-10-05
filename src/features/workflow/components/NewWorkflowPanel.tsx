import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type { WorkflowModeDto } from '@/lib/tauri/commands';
import type { WorkflowTemplate } from '../types';
import styles from './Workflow.module.css';

export interface NewWorkflowChoice {
  templateId: string | null;
  mode: WorkflowModeDto;
  name: string;
  task: string;
}

interface Props {
  templates: WorkflowTemplate[];
  /** The template Automatic mode proposes for a task. */
  recommend: (task: string) => Promise<string>;
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
export function NewWorkflowPanel({ templates, recommend, error, onCreate, onCancel }: Props) {
  const { t } = useI18n();
  const [mode, setMode] = useState<WorkflowModeDto>('automatic');
  const [task, setTask] = useState('');
  const [name, setName] = useState('');
  const [templateId, setTemplateId] = useState<string>('');
  const [recommended, setRecommended] = useState<string | null>(null);

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
    t(`workflow.template.${template.id}` as TranslationKey);

  return (
    <form
      className={styles.newPanel}
      aria-label={t('workflow.new.title')}
      onSubmit={(event) => {
        event.preventDefault();
        onCreate({ templateId: chosen, mode, name: name.trim(), task: task.trim() });
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
        <label className={styles.field}>
          <span className={styles.fieldLabel}>{t('workflow.new.template')}</span>
          <select
            className={styles.control}
            value={templateId}
            onChange={(e) => {
              setTemplateId(e.target.value);
            }}
          >
            <option value="">{t('workflow.new.blank')}</option>
            {templates.map((template) => (
              <option key={template.id} value={template.id}>
                {label(template)}
              </option>
            ))}
          </select>
        </label>
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
