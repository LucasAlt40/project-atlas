import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { useI18n } from '@/i18n/I18nProvider';
import styles from './Specs.module.css';

interface Props {
  questions: string[];
  /** The file(s) the agent will change. */
  targets: string[];
  busy: boolean;
  onSend: (note: string) => void;
  onClose: () => void;
}

/**
 * What the agent is told when the person answers the open questions of a document: each answer
 * next to its question, and the ones left blank named as still open, so none is guessed.
 */
export function answersNote(questions: string[], answers: string[]): string {
  const answered: string[] = [];
  const open: string[] = [];
  questions.forEach((question, index) => {
    const answer = (answers[index] ?? '').trim();
    if (answer === '') open.push(`- ${question}`);
    else answered.push(`Question: ${question}\nAnswer: ${answer}`);
  });
  const parts = [
    "The person answered the document's open questions. Write each answer into the document where it belongs and remove the answered question from Open Questions. Do not invent anything beyond what the answers say.",
    answered.join('\n\n'),
  ];
  if (open.length > 0) {
    parts.push(`Not answered yet, keep these in Open Questions:\n${open.join('\n')}`);
  }
  return parts.join('\n\n');
}

/** One field per open question: answering is filling in the blanks, not editing a text. */
export function AnswerQuestionsDialog({ questions, targets, busy, onSend, onClose }: Props) {
  const { t } = useI18n();
  const [answers, setAnswers] = useState<string[]>(() => questions.map(() => ''));
  const given = answers.filter((a) => a.trim() !== '').length;
  return (
    <Modal label={t('specs.answers.title')} onClose={onClose} placement="side">
      <form
        className={styles.form}
        onSubmit={(event) => {
          event.preventDefault();
          if (given > 0 && !busy) onSend(answersNote(questions, answers));
        }}
      >
        <h2 className={styles.dialogTitle}>{t('specs.answers.title')}</h2>
        <p className={styles.muted}>{t('specs.answers.intro')}</p>
        <p className={styles.mono}>{targets.join(' · ')}</p>
        <div className={styles.formBody}>
          {questions.map((question, index) => (
            <label key={question} className={styles.field}>
              <span className={styles.questionText}>
                {String(index + 1)}. {question}
              </span>
              <textarea
                rows={3}
                value={answers[index] ?? ''}
                maxLength={2000}
                placeholder={t('specs.answers.placeholder')}
                autoFocus={index === 0}
                onChange={(event) => {
                  setAnswers((current) =>
                    current.map((a, i) => (i === index ? event.target.value : a)),
                  );
                }}
              />
            </label>
          ))}
        </div>
        <p className={styles.muted}>
          {t('specs.answers.count', { given, total: questions.length })}
        </p>
        <div className={styles.dialogActions}>
          <Button variant="secondary" onClick={onClose}>
            {t('common.cancel')}
          </Button>
          <Button type="submit" disabled={given === 0 || busy}>
            {t('specs.answers.send')}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
