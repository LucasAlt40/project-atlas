import type { ReactNode } from 'react';
import { Icon, type IconName } from './Icon';
import styles from './Callout.module.css';

export type CalloutTone = 'info' | 'warning' | 'danger' | 'policy' | 'success';

const ICON: Record<CalloutTone, IconName> = {
  info: 'info',
  warning: 'warning',
  danger: 'warning',
  policy: 'shield',
  success: 'check',
};

interface Props {
  tone?: CalloutTone;
  /** Replaces the tone's icon (a lock for a privacy note). */
  icon?: IconName;
  title?: ReactNode;
  /** One action, at the end of the callout. */
  action?: ReactNode;
  /** `role="alert"` for errors the person must notice; `status` for live news. */
  role?: 'alert' | 'status' | 'note';
  children?: ReactNode;
}

/** A notice in the flow of a page: info, warning, error or policy, with at most one action. */
export function Callout({ tone = 'info', icon, title, action, role, children }: Props) {
  return (
    <div className={styles.callout} data-tone={tone} {...(role ? { role } : {})}>
      <span className={styles.icon} aria-hidden="true">
        <Icon name={icon ?? ICON[tone]} size={16} />
      </span>
      <div className={styles.content}>
        {title !== undefined && <div className={styles.title}>{title}</div>}
        {children !== undefined && <div className={styles.body}>{children}</div>}
      </div>
      {action !== undefined && <div className={styles.action}>{action}</div>}
    </div>
  );
}
