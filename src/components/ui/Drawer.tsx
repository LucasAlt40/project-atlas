import { useEffect, useRef, type ReactNode } from 'react';
import { useT } from '@/i18n/I18nProvider';
import { Icon } from './Icon';
import styles from './Drawer.module.css';

interface Props {
  /** Accessible name of the drawer. */
  label: string;
  onClose: () => void;
  /** The heading: a title and what goes next to it (a state badge). */
  title: ReactNode;
  /** A line under the title (what the object is, in a few words). */
  subtitle?: ReactNode;
  /** Tabs under the header; they stay in place while the body scrolls. */
  tabs?: ReactNode;
  /** The fixed footer: the drawer's actions. */
  footer?: ReactNode;
  /** 420, 520 (default) or 560px. */
  width?: 'sm' | 'md' | 'lg';
  children: ReactNode;
}

/**
 * A panel that slides over the right of the app, over a dimmed backdrop: details of one object,
 * or an edit form, without leaving the screen behind. Closes on Escape or a click on the backdrop.
 */
export function Drawer({
  label,
  onClose,
  title,
  subtitle,
  tabs,
  footer,
  width = 'md',
  children,
}: Props) {
  const t = useT();
  const panel = useRef<HTMLDivElement>(null);
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') onClose();
    }
    document.addEventListener('keydown', onKeyDown);
    return () => {
      document.removeEventListener('keydown', onKeyDown);
    };
  }, [onClose]);
  useEffect(() => {
    panel.current?.focus();
  }, []);

  return (
    <div
      className={styles.backdrop}
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        ref={panel}
        className={styles.drawer}
        data-width={width}
        role="dialog"
        aria-modal="true"
        aria-label={label}
        tabIndex={-1}
      >
        <header className={styles.header}>
          <div className={styles.titleRow}>
            <h2 className={styles.title}>{title}</h2>
            <button
              type="button"
              className={styles.close}
              aria-label={t('common.close')}
              title={t('common.close')}
              onClick={onClose}
            >
              <Icon name="close" size={18} />
            </button>
          </div>
          {subtitle !== undefined && <div className={styles.subtitle}>{subtitle}</div>}
        </header>
        {tabs !== undefined && <div className={styles.tabs}>{tabs}</div>}
        <div className={styles.body}>{children}</div>
        {footer !== undefined && <footer className={styles.footer}>{footer}</footer>}
      </div>
    </div>
  );
}
