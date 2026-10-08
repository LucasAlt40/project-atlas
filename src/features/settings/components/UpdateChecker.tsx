import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { errorMessage } from '@/i18n/messages';
import { useI18n } from '@/i18n/I18nProvider';
import { checkForUpdate, installUpdate } from '../services/updateService';
import styles from './UpdateChecker.module.css';

type State =
  | { status: 'idle' }
  | { status: 'checking' }
  | { status: 'up-to-date' }
  | { status: 'available'; version: string }
  | { status: 'installing' }
  | { status: 'installed' }
  | { status: 'error'; message: string };

/** The "check for updates" control: asks first, installs only when the person agrees. */
export function UpdateChecker() {
  const { t } = useI18n();
  const [state, setState] = useState<State>({ status: 'idle' });

  const check = async () => {
    setState({ status: 'checking' });
    try {
      const update = await checkForUpdate();
      setState(
        update ? { status: 'available', version: update.version } : { status: 'up-to-date' },
      );
    } catch (error) {
      setState({ status: 'error', message: errorMessage(t, error) });
    }
  };

  const install = async () => {
    setState({ status: 'installing' });
    try {
      await installUpdate();
      setState({ status: 'installed' });
    } catch (error) {
      setState({ status: 'error', message: errorMessage(t, error) });
    }
  };

  const busy = state.status === 'checking' || state.status === 'installing';

  return (
    <div className={styles.root}>
      <div className={styles.actions}>
        <Button variant="secondary" onClick={() => void check()} disabled={busy}>
          {state.status === 'checking' ? t('settings.updateChecking') : t('settings.updateCheck')}
        </Button>
        {state.status === 'available' || state.status === 'installing' ? (
          <Button onClick={() => void install()} disabled={busy}>
            {state.status === 'installing'
              ? t('settings.updateInstalling')
              : t('settings.updateInstall')}
          </Button>
        ) : null}
      </div>
      <p className={styles.status} role={state.status === 'error' ? 'alert' : 'status'}>
        {state.status === 'up-to-date' && t('settings.updateUpToDate')}
        {state.status === 'available' && t('settings.updateAvailable', { version: state.version })}
        {state.status === 'installed' && t('settings.updateInstalled')}
        {state.status === 'error' && state.message}
      </p>
    </div>
  );
}
