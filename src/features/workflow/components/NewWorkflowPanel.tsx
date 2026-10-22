import { useEffect, useState } from 'react';
import { ActionBar } from '@/components/ui/ActionBar';
import { Button } from '@/components/ui/Button';
import { Steps } from '@/components/ui/Steps';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type { ToolPlanItemDto, WorkflowModeDto } from '@/lib/tauri/commands';
import type { WorkflowTemplate } from '../types';
import { canGrantTools } from '../model/templateView';
import { StartChoice, type StartFrom } from '@/features/specs/components/StartChoice';
import type { SpecSummary } from '@/features/specs/types';
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
  /**
   * The specs a run could be started from, when the Specs module is on: the form then asks how to
   * start, vibe coding or a spec. Absent, the form is what it always was.
   */
  specs?: SpecSummary[];
  startFrom?: StartFrom;
  onStartFrom?: (value: StartFrom) => void;
  onNewSpec?: () => void;
}

interface CardsProps {
  templates: WorkflowTemplate[];
  selected: string;
  recommended?: string | null;
  /** A card for starting from nothing. */
  blank?: boolean;
  label: (template: WorkflowTemplate) => string;
  onSelect: (id: string) => void;
}

/** One card per workflow to start from: the Atlas ones first, then the person's own. */
function TemplateCards({ templates, selected, recommended, blank, label, onSelect }: CardsProps) {
  const { t } = useI18n();
  const ordered = [
    ...templates.filter((template) => template.kind === 'builtin'),
    ...templates.filter((template) => template.kind === 'custom'),
  ];
  return (
    <div className={styles.templateGrid}>
      {ordered.map((template) => (
        <label
          key={template.id}
          className={styles.templateCard}
          data-selected={selected === template.id}
        >
          <input
            type="radio"
            name="workflow-template"
            value={template.id}
            checked={selected === template.id}
            onChange={() => {
              onSelect(template.id);
            }}
          />
          <span className={styles.templateName}>
            <span>{label(template)}</span>
            <span className={styles.templateBadges}>
              {template.id === recommended && (
                <span className={styles.tag} data-kind="recommended">
                  {t('workflow.new.recommendedBadge')}
                </span>
              )}
              {template.kind === 'custom' && (
                <span className={styles.tag} data-kind="custom">
                  {t('workflow.template.badge.custom')}
                </span>
              )}
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
      {blank && (
        <label className={styles.templateCard} data-selected={selected === ''}>
          <input
            type="radio"
            name="workflow-template"
            value=""
            checked={selected === ''}
            onChange={() => {
              onSelect('');
            }}
          />
          <span className={styles.templateName}>{t('workflow.new.blank')}</span>
          <span className={styles.muted}>{t('workflow.new.blankHelp')}</span>
        </label>
      )}
    </div>
  );
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
  specs,
  startFrom,
  onStartFrom,
  onNewSpec,
}: Props) {
  const { t } = useI18n();
  const [mode, setMode] = useState<WorkflowModeDto>('automatic');
  const [task, setTask] = useState('');
  const [name, setName] = useState('');
  const [templateId, setTemplateId] = useState<string>('');
  // Automatic mode follows the recommendation until the person picks another workflow.
  const [override, setOverride] = useState<string | null>(null);
  const [recommended, setRecommended] = useState<string | null>(null);
  const [applyTools, setApplyTools] = useState(true);
  const [plan, setPlan] = useState<{ templateId: string; items: ToolPlanItemDto[] } | null>(null);
  /** 1 the task and how to start, 2 the workflow to start from, 3 its name. */
  const [step, setStep] = useState<1 | 2 | 3>(1);

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
  const chosen = mode === 'automatic' ? (override ?? recommended ?? null) : templateId || null;
  const label = (template: WorkflowTemplate) =>
    template.kind === 'custom'
      ? template.name
      : t(`workflow.template.${template.id}` as TranslationKey);
  const shown = templates.find((template) => template.id === chosen);

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

  const startLabel =
    startFrom?.kind === 'spec' ? t('specs.startFrom.spec') : t('specs.startFrom.vibe');
  const summary = (
    <p className={styles.newSummary}>
      <span>
        {t('workflow.new.summary', {
          task: task.trim() || t('workflow.new.noTask'),
          start: startLabel,
          mode: t(`workflow.mode.${mode}` as TranslationKey),
        })}
      </span>
      <button
        type="button"
        className={styles.link}
        onClick={() => {
          setStep(1);
        }}
      >
        {t('workflow.new.change')}
      </button>
    </p>
  );

  return (
    <form
      className={styles.newPanel}
      aria-label={t('workflow.new.title')}
      onSubmit={(event) => {
        event.preventDefault();
        if (step < 3) {
          setStep(step === 1 ? 2 : 3);
          return;
        }
        onCreate({
          templateId: chosen,
          mode,
          name: name.trim(),
          task: task.trim(),
          applyTools: applyTools && canGrantTools(plan?.templateId === chosen ? plan.items : []),
        });
      }}
    >
      <Steps
        label={t('workflow.new.steps')}
        steps={[
          { label: t('workflow.new.step.task'), state: step === 1 ? 'current' : 'done' },
          {
            label: t('workflow.new.step.template'),
            state: step === 2 ? 'current' : step > 2 ? 'done' : 'todo',
          },
          { label: t('workflow.new.step.name'), state: step === 3 ? 'current' : 'todo' },
        ]}
      />

      {step === 1 && (
        <div className={styles.newStep}>
          {specs && startFrom && onStartFrom && onNewSpec && (
            <StartChoice
              name="start-from-new"
              specs={specs}
              value={startFrom}
              onChange={onStartFrom}
              onNewSpec={onNewSpec}
            />
          )}
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

          {mode === 'automatic' && (
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
          )}
        </div>
      )}

      {step === 2 && (
        <>
          {summary}
          {mode === 'automatic' && proposal && (
            <p className={styles.recommendation} role="status">
              {override === null
                ? t('workflow.new.recommended', { name: label(proposal) })
                : t('workflow.new.chosen', { name: label(shown ?? proposal) })}
              <span className={styles.muted}>
                {' '}
                — {t(`workflow.templateHelp.${(shown ?? proposal).id}` as TranslationKey)}
              </span>
            </p>
          )}
          <div className={styles.newGrid}>
            <div className={styles.newCards}>
              {mode === 'automatic' ? (
                <fieldset className={styles.templateGroup}>
                  <legend className={styles.fieldLabel}>{t('workflow.new.chooseOther')}</legend>
                  <TemplateCards
                    templates={templates}
                    selected={chosen ?? ''}
                    recommended={recommended}
                    label={label}
                    onSelect={(id) => {
                      setOverride(id === recommended ? null : id);
                    }}
                  />
                </fieldset>
              ) : (
                <fieldset className={styles.templateGroup}>
                  <legend className={styles.fieldLabel}>{t('workflow.new.template')}</legend>
                  <TemplateCards
                    templates={templates}
                    selected={templateId}
                    blank
                    label={label}
                    onSelect={setTemplateId}
                  />
                </fieldset>
              )}
            </div>
            <div className={styles.newPreview}>
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
                          setOverride(null);
                          onDeleteTemplate(shown.id);
                        },
                      }
                    : {})}
                />
              )}
            </div>
          </div>
        </>
      )}

      {step === 3 && (
        <>
          {summary}
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
        </>
      )}

      {error && (
        <p role="alert" className={styles.failure}>
          {error}
        </p>
      )}
      <ActionBar
        label={t('workflow.new.title')}
        start={
          onCancel ? (
            <Button type="button" variant="ghost" onClick={onCancel}>
              {t('common.cancel')}
            </Button>
          ) : undefined
        }
        end={
          <>
            {step > 1 && (
              <Button
                type="button"
                variant="secondary"
                onClick={() => {
                  setStep(step === 3 ? 2 : 1);
                }}
              >
                {t('workflow.new.back')}
              </Button>
            )}
            <Button type="submit" disabled={step >= 2 && mode === 'automatic' && chosen === null}>
              {step < 3 ? t('workflow.new.continue') : t('workflow.new.create')}
            </Button>
          </>
        }
      />
    </form>
  );
}
