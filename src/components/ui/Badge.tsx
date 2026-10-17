import type { ReactNode } from 'react';
import styles from './Badge.module.css';

/** What the badge says about the thing it labels; each tone maps onto one semantic token. */
export type BadgeTone = 'neutral' | 'accent' | 'success' | 'warning' | 'danger' | 'info' | 'policy';

interface Props {
  tone?: BadgeTone;
  /** Mono capitals, for technical tags (`.atlas/`, `LANGUAGE`); plain for human labels. */
  mono?: boolean;
  /** With `mono`: capitals (the default, for tags and states) or the text as written (counts, names). */
  caps?: boolean;
  /** A leading status dot. */
  dot?: boolean;
  title?: string;
  children: ReactNode;
}

/** The one badge of the app: roles, harness state, health, finding kinds. */
export function Badge({
  tone = 'neutral',
  mono = false,
  caps = true,
  dot = false,
  title,
  children,
}: Props) {
  return (
    <span
      className={styles.badge}
      data-tone={tone}
      data-mono={mono ? true : undefined}
      data-caps={caps ? undefined : false}
      {...(title ? { title } : {})}
    >
      {dot && <span className={styles.dot} aria-hidden="true" />}
      {children}
    </span>
  );
}
