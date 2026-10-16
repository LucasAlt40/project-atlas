import { useEffect, useId, useRef, useState } from 'react';
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
  items: OverflowItem[];
}

/** The "⋯" menu: the secondary and destructive actions of a row, out of the way until asked. */
export function OverflowMenu({ label, items }: Props) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLSpanElement>(null);
  const menuId = useId();

  useEffect(() => {
    if (!open) return;
    function onPointerDown(event: MouseEvent) {
      if (root.current && !root.current.contains(event.target as Node)) setOpen(false);
    }
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') setOpen(false);
    }
    document.addEventListener('mousedown', onPointerDown);
    document.addEventListener('keydown', onKeyDown);
    return () => {
      document.removeEventListener('mousedown', onPointerDown);
      document.removeEventListener('keydown', onKeyDown);
    };
  }, [open]);

  return (
    <span className={styles.root} ref={root}>
      <button
        type="button"
        className={styles.trigger}
        aria-label={label}
        title={label}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        onClick={() => {
          setOpen((current) => !current);
        }}
      >
        <Icon name="more" size={16} />
      </button>
      {open && (
        <div id={menuId} role="menu" className={styles.menu}>
          {items.map((item) => (
            <button
              key={item.label}
              type="button"
              role="menuitem"
              className={styles.item}
              data-danger={item.danger || undefined}
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
