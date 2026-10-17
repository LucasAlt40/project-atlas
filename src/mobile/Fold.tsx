import type { ReactNode } from 'react';
import styles from './Mobile.module.css';

/**
 * A block of the run that folds down to its title and one line about it. A native `details`:
 * it costs the page nothing, works without a script and keeps the keyboard and screen readers.
 */
export function Fold({
  title,
  line,
  open = false,
  children,
}: {
  title: string;
  /** One line about what is inside, shown while it is folded. */
  line?: string;
  open?: boolean;
  children: ReactNode;
}) {
  return (
    <details className={styles.fold} open={open}>
      <summary className={styles.foldHead}>
        <span className={styles.foldText}>
          <span className={styles.foldTitle}>{title}</span>
          {line && <span className={styles.foldLine}>{line}</span>}
        </span>
        <span className={styles.foldChevron} aria-hidden="true" />
      </summary>
      <div className={styles.foldBody}>{children}</div>
    </details>
  );
}
