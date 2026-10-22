import { useDismiss } from '@/hooks/useDismiss';
import { useId, useRef, useState } from 'react';
import { Icon, type IconName } from './Icon';
import styles from './OverflowMenu.module.css';

export interface OverflowItem {
  label: string;
  onSelect: () => void;
  icon?: IconName;
  /** A destructive action is drawn apart, in the danger tone. */
  danger?: boolean;
}

interface Props {
  /** Accessible name of the "⋯" button. */
  label: string;
  /** A visible name next to the dots: the menu then reads as a button ("⋯ Configuration"). */
  text?: string;
  items: OverflowItem[];
  /** Which edge of the trigger the menu lines up with. */
  align?: 'end' | 'start';
}

/** The "⋯" menu: the secondary and destructive actions of a row, out of the way until asked. */
export function OverflowMenu({ label, text, items, align = 'end' }: Props) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLSpanElement>(null);
  const menuId = useId();

  useDismiss(open, root, () => {
    setOpen(false);
  });

  return (
    <span className={styles.root} ref={root}>
      <button
        type="button"
        className={styles.trigger}
        data-text={text ? true : undefined}
        {...(text ? {} : { 'aria-label': label, title: label })}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        onClick={() => {
          setOpen((current) => !current);
        }}
      >
        <Icon name="more" size={16} />
        {text}
      </button>
      {open && (
        <div id={menuId} role="menu" className={styles.menu} data-align={align}>
          {items.map((item, index) => (
            <button
              key={item.label}
              data-separated={
                item.danger && index > 0 && !items[index - 1]?.danger ? true : undefined
              }
              type="button"
              role="menuitem"
              className={styles.item}
              data-danger={item.danger ? true : undefined}
              onClick={() => {
                setOpen(false);
                item.onSelect();
              }}
            >
              {item.icon && <Icon name={item.icon} size={14} />}
              {item.label}
            </button>
          ))}
        </div>
      )}
    </span>
  );
}
