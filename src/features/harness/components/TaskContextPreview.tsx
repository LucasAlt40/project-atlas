import { useState } from 'react';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import { useTaskContextPreview } from '../hooks/useTaskContextPreview';
import { included, isRule, leftOut, reductionPercent } from '../model/taskContext';
import { TaskContextModal } from './TaskContextModal';
import styles from './Harness.module.css';

const SHOWN_INCLUDED = 6;
const SHOWN_EXCLUDED = 4;

/**
 * Before an agent runs, what it would be told about the project for the task being typed: how
 * big, how it compares with the whole Harness, what is in and what is not. Optional and
 * inspection only: the user can just send. Nothing is shown for a project without a Harness.
 */
export function TaskContextPreview({
  workspaceId,
  agentId,
  task,
}: {
  workspaceId: string;
  agentId: string;
  task: string;
}) {
  const { t, language } = useI18n();
  const state = useTaskContextPreview(workspaceId, agentId, task);
  const [open, setOpen] = useState(false);
  if (state.status === 'idle' || state.status === 'error') return null;
  if (state.status === 'loading')
    return (
      <p className={styles.contextBar} role="status">
        {t('taskContext.choosing')}
      </p>
    );
  const { preview } = state;
  if (preview.status === 'missing' || !preview.context) return null;
  if (preview.status === 'invalid')
    return <p className={styles.contextBar}>{t('taskContext.invalid')}</p>;
  const context = preview.context;
  const number = (n: number) => n.toLocaleString(language);

  if (context.mode === 'fallback') {
    const reason = context.fallbackReason ?? 'unknown';
    return (
      <section className={styles.contextBar} aria-label={t('taskContext.title')}>
        <strong>{t('taskContext.title')}</strong>
        <p className={styles.muted}>
          {t('taskContext.fallback', {
            reason: t(`taskContext.fallback.${reason}` as TranslationKey),
          })}
        </p>
      </section>
    );
  }

  const chosen = included(context);
  const rules = chosen.filter(isRule).length;
  const labels = chosen.filter((e) => !isRule(e)).map((e) => e.item.label);
  const out = leftOut(context).map((e) => e.item.label);
  const reduced = reductionPercent(context);
  return (
    <section className={styles.contextBar} aria-label={t('taskContext.title')}>
      <div className={styles.contextHead}>
        <strong>{t('taskContext.title')}</strong>
        <span className={styles.muted}>
          {t('taskContext.chars', {
            selected: number(context.selectedChars),
            budget: number(context.budgetChars),
          })}
        </span>
        <button
          type="button"
          className={styles.contextView}
          onClick={() => {
            setOpen(true);
          }}
        >
          {t('taskContext.view')}
        </button>
      </div>
      <p className={styles.muted}>
        {t('taskContext.comparison', {
          full: number(context.fullHarnessChars),
          selected: number(context.selectedChars),
        })}
        {reduced > 0 && ` · ${t('taskContext.reduction', { percent: reduced })}`}
      </p>
      <ul className={styles.chips} aria-label={t('taskContext.included')}>
        {rules > 0 && <li className={styles.chipOk}>✓ {t('taskContext.rules')}</li>}
        {labels.slice(0, SHOWN_INCLUDED).map((label) => (
          <li key={label} className={styles.chipOk}>
            ✓ {label}
          </li>
        ))}
        {labels.length > SHOWN_INCLUDED && (
          <li className={styles.muted}>
            {t('taskContext.more', { count: labels.length - SHOWN_INCLUDED })}
          </li>
        )}
      </ul>
      {out.length > 0 && (
        <ul className={styles.chips} aria-label={t('taskContext.excluded')}>
          {out.slice(0, SHOWN_EXCLUDED).map((label) => (
            <li key={label} className={styles.chipOut}>
              ○ {label}
            </li>
          ))}
          {out.length > SHOWN_EXCLUDED && (
            <li className={styles.muted}>
              {t('taskContext.more', { count: out.length - SHOWN_EXCLUDED })}
            </li>
          )}
        </ul>
      )}
      <p className={styles.muted}>
        {t('taskContext.areasIncluded', { count: context.includedAreas.length })} ·{' '}
        {t('taskContext.areasExcluded', { count: context.excludedAreas.length })}
      </p>
      {context.truncated && <p className={styles.muted}>{t('taskContext.truncated')}</p>}
      {open && (
        <TaskContextModal
          context={context}
          task={task}
          onClose={() => {
            setOpen(false);
          }}
        />
      )}
    </section>
  );
}
