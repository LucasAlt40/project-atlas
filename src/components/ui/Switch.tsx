import type { ReactNode } from 'react';
import styles from './Switch.module.css';

interface Props {
  checked: boolean;
  onChange: (checked: boolean) => void;
  /** The visible label; it names the switch. */
  children?: ReactNode;
  /** Accessible name when there is no visible label. */
  label?: string;
  disabled?: boolean;
  /** Where the label sits: before the switch (settings rows) or after it (toolbars). */
  labelPosition?: 'before' | 'after';
  title?: string;
}

/** An on/off switch: the setting takes effect as soon as it is flipped. */
export function Switch({
  checked,
  onChange,
  children,
  label,
  disabled = false,
  labelPosition = 'after',
  title,
}: Props) {
  return (
    <label
      className={styles.root}
      data-position={labelPosition}
      data-disabled={disabled ? true : undefined}
      {...(title ? { title } : {})}
    >
      <input
        type="checkbox"
        role="switch"
        className={styles.input}
        checked={checked}
        disabled={disabled}
        {...(label ? { 'aria-label': label } : {})}
        onChange={(event) => {
          onChange(event.target.checked);
        }}
      />
      <span className={styles.track} aria-hidden="true">
        <span className={styles.thumb} />
      </span>
      {children !== undefined && <span className={styles.text}>{children}</span>}
    </label>
  );
}
