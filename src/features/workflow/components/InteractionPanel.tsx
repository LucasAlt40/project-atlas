import { useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Markdown } from '@/components/ui/Markdown';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type { InteractionAnswerDto } from '@/lib/tauri/commands';
import type { PendingInteraction } from '../types';
import styles from './Interaction.module.css';

/** The buttons whose meaning Atlas fixes; any other option is a suggestion and shows as given. */
const FIXED_OPTIONS = new Set(['approve', 'reject', 'allow', 'deny', 'yes', 'no']);

interface Props {
  interaction: PendingInteraction;
  /** The answer is on its way to the core. */
  busy: boolean;
  /** The core refused the last answer (not pending any more, not an option…). */
  error?: string | null;
  onAnswer: (answer: InteractionAnswerDto) => void;
  onCancelRun: () => void;
}

/**
 * "Action required": an agent has stopped and is waiting for the person. It cannot be missed
 * or mistaken for a log line. A clarification takes text; an approval or a permission takes its
 * two buttons. Answering lets the agent go on inside the run's own worktree: it applies nothing
 * to the project, which stays a separate review.
 */
export function InteractionPanel({ interaction, busy, error, onAnswer, onCancelRun }: Props) {
  const { t } = useI18n();
  const [text, setText] = useState('');
  const root = useRef<HTMLElement>(null);

  // Arriving here from "Action required" puts the question in front of the person.
  useEffect(() => {
    // jsdom (tests) has no layout and no scrollIntoView.
    if (typeof root.current?.scrollIntoView === 'function') {
      root.current.scrollIntoView({ block: 'nearest' });
    }
  }, [interaction.id]);

  const decision = interaction.kind !== 'clarification';
  const optionLabel = (id: string, label: string) =>
    FIXED_OPTIONS.has(id) ? t(`interaction.option.${id}` as TranslationKey) : label;
  const submit = () => {
    const answer = text.trim();
    if (answer) onAnswer({ text: answer });
  };

  return (
    <section
      ref={root}
      className={styles.panel}
      role="alert"
      aria-labelledby={`interaction-${interaction.id}`}
      data-kind={interaction.kind}
    >
      <h2 id={`interaction-${interaction.id}`} className={styles.title}>
        <span aria-hidden="true">⚠</span> {t('interaction.required')}
      </h2>
      <p className={styles.who}>{t('interaction.waiting', { step: interaction.stepLabel })}</p>
      <p className={styles.reason}>
        {t('interaction.reason', {
          reason: t(`interaction.kind.${interaction.kind}` as TranslationKey),
        })}
      </p>
      <blockquote className={styles.question}>{interaction.question}</blockquote>
      {interaction.document && (
        <details className={styles.document} open>
          <summary>{t('interaction.document')}</summary>
          <Markdown>{interaction.document}</Markdown>
        </details>
      )}
      {interaction.context && (
        <details className={styles.context}>
          <summary>{t('interaction.context')}</summary>
          <p>{interaction.context}</p>
        </details>
      )}

      {decision ? (
        <>
          <div className={styles.actions}>
            {interaction.options.map((option) => (
              <Button
                key={option.id}
                disabled={busy}
                onClick={() => {
                  onAnswer({ choice: option.id });
                }}
              >
                {optionLabel(option.id, option.label)}
              </Button>
            ))}
          </div>
          <p className={styles.note}>{t('interaction.worktreeNote')}</p>
        </>
      ) : (
        <>
          {interaction.options.length > 0 && (
            <div className={styles.actions} aria-label={t('interaction.suggestions')}>
              {interaction.options.map((option) => (
                <Button
                  key={option.id}
                  disabled={busy}
                  onClick={() => {
                    setText(option.id);
                  }}
                >
                  {option.label}
                </Button>
              ))}
            </div>
          )}
          <textarea
            className={styles.input}
            aria-label={t('interaction.answer')}
            placeholder={t('interaction.answer.placeholder')}
            value={text}
            disabled={busy}
            rows={3}
            onChange={(e) => {
              setText(e.target.value);
            }}
          />
          <div className={styles.actions}>
            <Button disabled={busy || text.trim() === ''} onClick={submit}>
              {t('interaction.send')}
            </Button>
          </div>
        </>
      )}

      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
      <div className={styles.footer}>
        <Button disabled={busy} onClick={onCancelRun}>
          {t('interaction.cancelRun')}
        </Button>
        <span className={styles.note}>{t('interaction.noAutoApply')}</span>
      </div>
    </section>
  );
}
