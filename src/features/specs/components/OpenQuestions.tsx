import { Button } from '@/components/ui/Button';
import { useI18n } from '@/i18n/I18nProvider';
import styles from './Specs.module.css';

interface Props {
  questions: string[];
  busy: boolean;
  onAnswer: () => void;
}

/**
 * What the document itself says it does not know. Shown whenever there is something to show,
 * whatever the person decided about the document: approving it does not make the gap go away.
 */
export function OpenQuestions({ questions, busy, onAnswer }: Props) {
  const { t } = useI18n();
  if (questions.length === 0) return null;
  return (
    <section className={styles.openQuestions} aria-label={t('specs.openQuestions.answer')}>
      <h3>{t('specs.openQuestions.title', { count: questions.length })}</h3>
      <ul>
        {questions.map((question) => (
          <li key={question}>{question}</li>
        ))}
      </ul>
      <p>{t('specs.openQuestions.note')}</p>
      <div className={styles.row}>
        <Button disabled={busy} onClick={onAnswer}>
          {t('specs.openQuestions.answer')}
        </Button>
      </div>
    </section>
  );
}
