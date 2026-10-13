import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { useI18n } from '@/i18n/I18nProvider';
import type { SpecQuestion } from '../types';
import styles from './Specs.module.css';

interface Props {
  question: SpecQuestion;
  busy: boolean;
  onAnswer: (choice: string | null, answer: string | null) => void;
}

/** What an agent needs from the person before it can go on. */
export function QuestionCard({ question, busy, onAnswer }: Props) {
  const { t } = useI18n();
  const [text, setText] = useState('');
  return (
    <section className={styles.question} aria-label={t('specs.question.label')}>
      <header>
        <Icon name="help" size={16} />
        <h3>{t('specs.question.title')}</h3>
      </header>
      <p className={styles.questionText}>{question.question}</p>
      {question.context && <p className={styles.muted}>{question.context}</p>}
      {question.options.length > 0 && (
        <div className={styles.row}>
          {question.options.map((option) => (
            <Button
              key={option.id}
              variant="secondary"
              disabled={busy}
              onClick={() => {
                onAnswer(option.id, text.trim() || null);
              }}
            >
              {option.label}
            </Button>
          ))}
        </div>
      )}
      <label className={styles.field}>
        <span>{t('specs.question.answer')}</span>
        <textarea
          rows={3}
          value={text}
          maxLength={2000}
          onChange={(event) => {
            setText(event.target.value);
          }}
        />
      </label>
      <div className={styles.dialogActions}>
        <Button
          disabled={busy || text.trim() === ''}
          onClick={() => {
            onAnswer(null, text.trim());
          }}
        >
          {t('specs.question.send')}
        </Button>
      </div>
    </section>
  );
}
