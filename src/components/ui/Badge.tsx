import type { ReactNode } from 'react';
import styles from './Badge.module.css';

/** What the badge says about the thing it labels; each tone maps onto one semantic token. */
export type BadgeTone =
  'neutral' | 'accent' | 'success' | 'warning' | 'danger' | 'info' | 'running' | 'policy' | 'idle';

interface Props {
  tone?: BadgeTone;
  /** Mono, for technical values (`.atlas/`, a model id, a count); plain for human labels. */
  mono?: boolean;
  /** A leading status dot: the badge says the state of something. */
  dot?: boolean;
  title?: string;
  children: ReactNode;
}

/**
 * The one badge of the app: a 4px-radius chip with the tone at 14% behind and 40% around.
 * States carry a dot; categories (roles, kinds) are neutral and have none.
 */
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
