import type { ReactNode } from 'react';
import { Icon, type IconName } from './Icon';
import styles from './Tabs.module.css';

export interface TabItem<Id extends string> {
  id: Id;
  label: string;
  icon?: IconName;
  /** A count after the label ("Execuções 3"). */
  count?: number;
  disabled?: boolean;
  /** Extra hint on hover (why a tab is disabled). */
  title?: string;
}

interface Props<Id extends string> {
  /** Accessible name of the tab list. */
  label: string;
  items: readonly TabItem<Id>[];
  active: Id;
  onChange: (id: Id) => void;
  /** Prefix for the tab and panel ids, so a panel can name its tab (`aria-labelledby`). */
  idPrefix?: string;
  /** Something at the end of the row (an action that belongs to the tabs). */
  trailing?: ReactNode;
}

/** Underlined tabs: one row of views of the same object, the current one in the accent. */
export function Tabs<Id extends string>({
  label,
  items,
  active,
  onChange,
  idPrefix,
  trailing,
}: Props<Id>) {
  return (
    <div className={styles.row}>
      <div className={styles.tabs} role="tablist" aria-label={label}>
        {items.map((item) => (
          <button
            key={item.id}
            type="button"
            role="tab"
            className={styles.tab}
            {...(idPrefix
              ? {
                  id: `${idPrefix}-tab-${item.id}`,
                  'aria-controls': `${idPrefix}-panel-${item.id}`,
                }
              : {})}
            aria-selected={item.id === active}
            tabIndex={item.id === active ? 0 : -1}
            disabled={item.disabled}
            {...(item.title ? { title: item.title } : {})}
            onClick={() => {
              onChange(item.id);
            }}
            onKeyDown={(event) => {
              if (event.key !== 'ArrowRight' && event.key !== 'ArrowLeft') return;
              const enabled = items.filter((i) => !i.disabled);
              const at = enabled.findIndex((i) => i.id === active);
              const step = event.key === 'ArrowRight' ? 1 : -1;
              const next = enabled[(at + step + enabled.length) % enabled.length];
              if (next) {
                onChange(next.id);
                const list = event.currentTarget.parentElement;
                requestAnimationFrame(() => {
                  list
                    ?.querySelector<HTMLButtonElement>('[role="tab"][aria-selected="true"]')
                    ?.focus();
                });
              }
            }}
          >
            {item.icon && <Icon name={item.icon} size={16} />}
            <span className={styles.label}>{item.label}</span>
            {item.count !== undefined && <span className={styles.count}>{item.count}</span>}
          </button>
        ))}
      </div>
      {trailing !== undefined && <div className={styles.trailing}>{trailing}</div>}
    </div>
  );
}
