import type { ReactNode } from 'react';
import styles from './ActionBar.module.css';

interface Props {
  /** On the left: a quiet way out ("Cancelar"), or a status ("Alterações não salvas"). */
  start?: ReactNode;
  /** On the right: the secondary actions, then the primary one. */
  end: ReactNode;
  /** `page`: fixed at the bottom of a page; `inline`: closes a block. */
  variant?: 'page' | 'inline';
  label?: string;
}

/** The footer of a form or a flow: always visible, the primary action at the right. */
export function ActionBar({ start, end, variant = 'page', label }: Props) {
  return (
    <div
      className={styles.bar}
      data-variant={variant}
      {...(label ? { role: 'group', 'aria-label': label } : {})}
    >
      <div className={styles.start}>{start}</div>
      <div className={styles.end}>{end}</div>
    </div>
  );
}
