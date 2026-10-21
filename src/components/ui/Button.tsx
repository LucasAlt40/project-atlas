import type { ButtonHTMLAttributes } from 'react';
import styles from './Button.module.css';

/**
 * `primary` is the one action that moves the work forward (one per area); `secondary` is
 * everything else a page or dialog offers; `ghost` is a quiet inline action; `danger` is a
 * destructive one.
 */
export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  /** `md` (32px) is the default; `sm` (28px) for dense rows, toolbars and card headers. */
  size?: 'md' | 'sm';
}

export function Button({
  className,
  type = 'button',
  variant = 'primary',
  size = 'md',
  ...rest
}: Props) {
  const classes = [styles.button, className].filter(Boolean).join(' ');
  return (
    <button
      type={type}
      className={classes}
      data-variant={variant}
      data-size={size === 'sm' ? 'sm' : undefined}
      {...rest}
    />
  );
}
