import type { ButtonHTMLAttributes } from 'react';
import styles from './AppShell.module.css';

/** The list a header indicator opens: `popoverRoot`, `popover`, `popoverItem`, `popoverMeta`. */
export const headerPopover = {
  root: styles.popoverRoot,
  list: styles.popover,
  item: styles.popoverItem,
  meta: styles.popoverMeta,
};

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  /** `warning`: agents waiting for the person; `running`: agents at work. */
  tone: 'warning' | 'running';
  /** What a narrow window shows instead of the words: the number alone. */
  count: number;
}

/** An indicator of the header ("1 agente aguardando você"): a toned chip that opens a list. */
export function HeaderChip({ tone, count, children, className, ...rest }: Props) {
  return (
    <button
      type="button"
      className={[styles.chip, className].filter(Boolean).join(' ')}
      data-tone={tone}
      {...rest}
    >
      <span className={styles.chipDot} aria-hidden="true" />
      <span className={styles.chipText}>{children}</span>
      <span className={styles.chipCount} aria-hidden="true">
        {count}
      </span>
    </button>
  );
}
