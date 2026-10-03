import { useEffect, type ReactNode } from 'react';
import { useT } from '@/i18n/I18nProvider';
import styles from './Modal.module.css';

interface Props {
  /** Accessible name of the dialog. */
  label: string;
  onClose: () => void;
  /** `side` slides in from the right (details panels); `center` is a classic dialog. */
  placement?: 'center' | 'side';
  children: ReactNode;
}

/** A dialog over the app. Closes on Escape or a click on the backdrop. */
export function Modal({ label, onClose, placement = 'center', children }: Props) {
  const t = useT();
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') onClose();
    }
    document.addEventListener('keydown', onKeyDown);
    return () => {
      document.removeEventListener('keydown', onKeyDown);
    };
  }, [onClose]);

  return (
    <div
      className={styles.backdrop}
      data-placement={placement}
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div className={styles.dialog} role="dialog" aria-modal="true" aria-label={label}>
        <button
          type="button"
          className={styles.close}
          aria-label={t('common.close')}
          onClick={onClose}
        >
          ✕
        </button>
        {children}
      </div>
    </div>
  );
}
