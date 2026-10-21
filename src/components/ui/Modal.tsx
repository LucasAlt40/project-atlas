import { useEffect, type ReactNode } from 'react';
import { useT } from '@/i18n/I18nProvider';
import { Icon } from './Icon';
import styles from './Modal.module.css';

export type ModalSize = 'sm' | 'md' | 'lg' | 'xl';

interface Props {
  /** Accessible name of the dialog. */
  label: string;
  onClose: () => void;
  /** `side` slides in from the right (prefer `Drawer` for new panels); `center` is a dialog. */
  placement?: 'center' | 'side';
  /** P 420px, M 640px (default), G 960px; `xl` for a full inspector. */
  size?: ModalSize;
  /** The heading. With it, the dialog draws its own header, a scrolling body and `footer`. */
  title?: ReactNode;
  /** Fixed at the bottom: "Cancelar" and the primary action, at the right. */
  footer?: ReactNode;
  children: ReactNode;
}

/** A dialog over the app: never more than one at a time. Closes on Escape or the backdrop. */
export function Modal({
  label,
  onClose,
  placement = 'center',
  size = 'md',
  title,
  footer,
  children,
}: Props) {
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

  const close = (
    <button
      type="button"
      className={styles.close}
      aria-label={t('common.close')}
      title={t('common.close')}
      onClick={onClose}
    >
      <Icon name="close" size={18} />
    </button>
  );

  return (
    <div
      className={styles.backdrop}
      data-placement={placement}
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        className={styles.dialog}
        data-size={size}
        data-framed={title !== undefined ? true : undefined}
        role="dialog"
        aria-modal="true"
        aria-label={label}
      >
        {title !== undefined ? (
          <>
            <header className={styles.header}>
              <h2 className={styles.title}>{title}</h2>
              {close}
            </header>
            <div className={styles.body}>{children}</div>
            {footer !== undefined && <footer className={styles.footer}>{footer}</footer>}
          </>
        ) : (
          <>
            {close}
            {children}
          </>
        )}
      </div>
    </div>
  );
}
