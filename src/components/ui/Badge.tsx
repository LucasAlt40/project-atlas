import type { ReactNode } from 'react';
import styles from './Badge.module.css';

/** What the badge says about the thing it labels; each tone maps onto one semantic token. */
export type BadgeTone = 'neutral' | 'accent' | 'success' | 'warning' | 'danger' | 'info';

interface Props {
  tone?: BadgeTone;
  /** Mono capitals, for technical tags (`.atlas/`, `LANGUAGE`); plain for human labels. */
  mono?: boolean;
  /** A leading status dot. */
  dot?: boolean;
  title?: string;
  children: ReactNode;
}

/** The one badge of the app: roles, harness state, health, finding kinds. */
export function Badge({ tone = 'neutral', mono = false, dot = false, title, children }: Props) {
  return (
    <span
      className={styles.badge}
      data-tone={tone}
      data-mono={mono ? true : undefined}
      {...(title ? { title } : {})}
    >
      {dot && <span className={styles.dot} aria-hidden="true" />}
      {children}
    </span>
  );
}
