import { useT } from '@/i18n/I18nProvider';
import styles from './Harness.module.css';

export interface Problem {
  message: string;
  /** The technical reason, when the core gave one. */
  detail?: string | undefined;
}

/** An error in the user's language plus, folded away, the exact reason it happened. */
export function ErrorBox({ problem }: { problem: Problem }) {
  const t = useT();
  return (
    <div role="alert" className={styles.error}>
      <p className={styles.errorText}>{problem.message}</p>
      {problem.detail && (
        <details>
          <summary>{t('harness.error.details')}</summary>
          <pre className={styles.detail}>{problem.detail}</pre>
        </details>
      )}
    </div>
  );
}
