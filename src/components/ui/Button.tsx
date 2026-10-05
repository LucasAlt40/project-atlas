import type { ButtonHTMLAttributes } from 'react';
import styles from './Button.module.css';

/**
 * `primary` is the one action that moves the work forward; `secondary` is everything else a
 * dialog or toolbar offers (cancel, close, redetect); `ghost` is a quiet inline action; `danger`
 * is a destructive one.
 */
export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
}

export function Button({ className, type = 'button', variant = 'primary', ...rest }: Props) {
  const classes = [styles.button, className].filter(Boolean).join(' ');
  return <button type={type} className={classes} data-variant={variant} {...rest} />;
}
