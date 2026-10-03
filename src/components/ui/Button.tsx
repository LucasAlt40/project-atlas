import type { ButtonHTMLAttributes } from 'react';
import styles from './Button.module.css';

export function Button({
  className,
  type = 'button',
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement>) {
  const classes = [styles.button, className].filter(Boolean).join(' ');
  return <button type={type} className={classes} {...rest} />;
}
