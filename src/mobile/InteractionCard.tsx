import { useState } from 'react';
import { Markdown } from '@/components/ui/Markdown';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import { api, ApiError } from './api';
import { clock } from './format';
import { CHOICE_KEY, KIND_KEY } from './labels';
import type { Applied, InteractionView, RunDetail } from './types';
import styles from './Mobile.module.css';

const PROBLEM_KEY: Record<string, TranslationKey> = {
  already_resolved: 'mobile.answer.alreadyResolved',
  desktop_only: 'mobile.answer.desktopOnly',
  invalid_answer: 'mobile.answer.invalid',
  run_not_live: 'mobile.answer.notLive',
  rate_limited: 'mobile.answer.rateLimited',
  unreachable: 'mobile.answer.unreachable',
  not_found: 'mobile.answer.notFound',
};

export const APPLIED_KEY: Record<Applied, TranslationKey> = {
  now: 'mobile.answer.sent',
  already: 'mobile.answer.already',
  pending: 'mobile.answer.pending',
  recorded: 'mobile.answer.recorded',
};

/** Which of a question's options lean on the person saying no (shown as the quieter button). */
const DECLINES = new Set(['reject', 'deny', 'no', 'redo']);

/** The words of an option: Atlas's own for the ones it fixed, the agent's for its suggestions. */
function useChoiceLabel() {
  const { t } = useI18n();
  return (id: string, fallback: string): string => {
    const key = CHOICE_KEY[id];
    return key ? t(key) : fallback;
  };
}

/** A question already answered: what was decided, by whom, when. */
function Settled({ interaction }: { interaction: InteractionView }) {
  const { t, language } = useI18n();
  const label = useChoiceLabel();
  const choice = interaction.options.find((o) => o.id === interaction.choice);
  return (
    <details className={styles.settled}>
      <summary>
        <span aria-hidden="true">✓ </span>
        {interaction.step} ·{' '}
        {interaction.auto
          ? t('mobile.settled.auto')
          : choice
            ? label(choice.id, choice.label)
            : t('mobile.settled.answered')}
      </summary>
      <p className={styles.muted}>{interaction.question}</p>
      {interaction.answer && <p className={styles.quote}>{interaction.answer}</p>}
      {interaction.answeredAt && (
        <p className={styles.fine}>{clock(interaction.answeredAt, language)}</p>
      )}
    </details>
  );
}

/**
 * A question waiting for a person. The answer is sent to the computer, which checks it against the
 * question; what the page shows afterwards is what the computer confirmed, not what was tapped.
 */
export function InteractionCard({
  run,
  interaction,
  onSettled,
  onStale,
}: {
  run: RunDetail;
  interaction: InteractionView;
  onSettled: (run: RunDetail, applied: Applied) => void;
  /** The computer says the question is not what the page thinks: read the run again. */
  onStale: () => void;
}) {
  const { t } = useI18n();
  const label = useChoiceLabel();
  const [text, setText] = useState('');
  const [busy, setBusy] = useState<string | null>(null);
  const [problem, setProblem] = useState<TranslationKey | null>(null);
  const [expanded, setExpanded] = useState(interaction.kind === 'plan');
  const [asking, setAsking] = useState<string | null>(null);

  if (interaction.status !== 'pending') return <Settled interaction={interaction} />;

  const { remote } = interaction;
  const allowed = interaction.options.filter(
    (option) => remote.choices === null || remote.choices.includes(option.id),
  );
  const isText = interaction.kind === 'clarification';
  // `revise` is the one answer that cannot be sent without words.
  const needsWords = (id: string) => interaction.kind === 'plan' && id === 'revise';

  const send = async (choice: string | undefined) => {
    setBusy(choice ?? 'text');
    setProblem(null);
    try {
      const answered = await api.answer(run.id, interaction.id, {
        ...(choice ? { choice } : {}),
        ...(text.trim() ? { text: text.trim() } : {}),
      });
      onSettled(answered.run, answered.applied);
    } catch (error) {
      const code = error instanceof ApiError ? error.code : 'unreachable';
      setProblem(PROBLEM_KEY[code] ?? 'mobile.answer.failed');
      // The computer knows better than the page what became of the question.
      if (code === 'already_resolved' || code === 'run_not_live' || code === 'not_found') onStale();
    } finally {
      setBusy(null);
    }
  };

  const choose = (id: string) => {
    if (needsWords(id) && !text.trim()) {
      setAsking(id);
      return;
    }
    void send(id);
  };

  const headingId = `question-${interaction.id}`;
  return (
    <section className={styles.question} aria-labelledby={headingId}>
      <p className={styles.kind}>
        {t(KIND_KEY[interaction.kind])}
        <span className={styles.muted}>
          {' · '}
          {interaction.step}
          {interaction.agent ? ` · ${interaction.agent}` : ''}
        </span>
      </p>
      <h3 id={headingId} className={styles.questionText}>
        {interaction.question}
      </h3>
      {interaction.context && <p className={styles.muted}>{interaction.context}</p>}
      {interaction.document && (
        <div className={styles.document} data-expanded={expanded}>
          <Markdown>{interaction.document}</Markdown>
          <button
            type="button"
            className={styles.expand}
            aria-expanded={expanded}
            onClick={() => {
              setExpanded((value) => !value);
            }}
          >
            {expanded ? t('mobile.question.collapse') : t('mobile.question.expand')}
          </button>
        </div>
      )}

      {!remote.canAnswer ? (
        <p className={styles.callout} data-tone="attention" role="note">
          {t(
            remote.desktopOnly === 'guardrail'
              ? 'mobile.question.guardrail'
              : 'mobile.question.desktopOnly',
          )}
        </p>
      ) : (
        <div className={styles.answer}>
          {remote.desktopOnly === 'grants_authority' && (
            <p className={styles.callout} data-tone="attention" role="note">
              {t('mobile.question.refuseOnly')}
            </p>
          )}
          {(isText || interaction.kind === 'plan' || asking) && (
            <>
              <label htmlFor={`text-${interaction.id}`} className={styles.label}>
                {isText
                  ? t('mobile.question.yourAnswer')
                  : asking
                    ? t('mobile.question.whatToChange')
                    : t('mobile.question.noteOptional')}
              </label>
              <textarea
                id={`text-${interaction.id}`}
                className={styles.textarea}
                rows={isText ? 4 : 3}
                maxLength={2000}
                value={text}
                onChange={(event) => {
                  setText(event.target.value);
                }}
              />
            </>
          )}
          <div className={styles.choices}>
            {isText && (
              <button
                type="button"
                className={styles.primary}
                disabled={busy !== null || !text.trim()}
                onClick={() => {
                  void send(undefined);
                }}
              >
                {busy === 'text' ? t('mobile.question.sending') : t('mobile.question.send')}
              </button>
            )}
            {allowed.map((option, index) => (
              <button
                key={option.id}
                type="button"
                className={
                  !isText && index === 0 && !DECLINES.has(option.id)
                    ? styles.primary
                    : styles.secondary
                }
                data-decline={DECLINES.has(option.id) ? 'true' : undefined}
                disabled={busy !== null}
                onClick={() => {
                  choose(option.id);
                }}
              >
                {busy === option.id ? t('mobile.question.sending') : label(option.id, option.label)}
              </button>
            ))}
          </div>
          {asking && !text.trim() && (
            <p className={styles.fine}>{t('mobile.question.revisionNeedsWords')}</p>
          )}
        </div>
      )}

      {problem && (
        <p className={styles.error} role="alert">
          {t(problem)}
        </p>
      )}
    </section>
  );
}
