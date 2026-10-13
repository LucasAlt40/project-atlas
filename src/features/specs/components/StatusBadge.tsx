import type { ReactNode } from 'react';
import type { Tone } from '../model/status';
import styles from './Specs.module.css';

export function StatusBadge({ tone, children }: { tone: Tone; children: ReactNode }) {
  return (
    <span className={styles.badge} data-tone={tone}>
      <span className={styles.badgeDot} aria-hidden="true" />
      {children}
    </span>
  );
}
