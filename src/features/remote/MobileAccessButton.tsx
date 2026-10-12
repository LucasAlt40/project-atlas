import { useState } from 'react';
import { Icon } from '@/components/ui/Icon';
import { useT } from '@/i18n/I18nProvider';
import { MobileAccessDialog } from './MobileAccessDialog';
import { useRemoteAccess } from './useRemoteAccess';
import styles from './Remote.module.css';

/**
 * In the header of every screen: where the phone's access is switched on, and a look at whether
 * it is on. A dot and a count say it without opening the dialog.
 */
export function MobileAccessButton() {
  const t = useT();
  const [open, setOpen] = useState(false);
  const { status } = useRemoteAccess(5000);
  const state = status?.state ?? 'disabled';
  const waiting = status?.waiting ?? 0;

  return (
    <>
      <button
        type="button"
        className={styles.button}
        title={t('remote.buttonTitle')}
        aria-haspopup="dialog"
        onClick={() => {
          setOpen(true);
        }}
      >
        <Icon name="phone" size={14} />
        {t('remote.button')}
        <span className={styles.dot} data-state={state} aria-hidden="true" />
        <span className={styles.srOnly}>{t(`remote.state.${state}`)}</span>
        {waiting > 0 && <span className={styles.badge}>{waiting}</span>}
      </button>
      {open && (
        <MobileAccessDialog
          onClose={() => {
            setOpen(false);
          }}
        />
      )}
    </>
  );
}
