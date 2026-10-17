import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { CollapsibleSection } from '@/components/ui/CollapsibleSection';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import type { PresenceSettingsDto } from '@/lib/tauri/commands';
import { getSettings, setPresenceSettings } from '../services/settingsService';
import styles from '../pages/SettingsPage.module.css';

const OFF: PresenceSettingsDto = { enabled: false, showNames: true, clientId: null };

/**
 * Discord Rich Presence (proof of concept): what Atlas may show on the person's profile. Off until
 * switched on; the names are a separate choice. What is shown is composed by the core.
 */
export function PresenceSettings() {
  const { t } = useI18n();
  const [settings, setSettings] = useState<PresenceSettingsDto | null>(null);
  const [clientId, setClientId] = useState('');
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    Promise.resolve(getSettings())
      .then((loaded) => {
        if (cancelled) return;
        const presence = loaded.presence ?? OFF;
        setSettings(presence);
        setClientId(presence.clientId ?? '');
      })
      .catch((error: unknown) => {
        if (!cancelled) setFailure(errorMessage(t, error));
      });
    return () => {
      cancelled = true;
    };
  }, [t]);

  async function change(next: PresenceSettingsDto) {
    setFailure(null);
    try {
      const saved = await setPresenceSettings(next);
      setSettings(saved.presence ?? next);
    } catch (error) {
      setFailure(errorMessage(t, error));
    }
  }

  if (!settings) return failure ? <p role="alert">{failure}</p> : null;

  return (
    <div className={styles.stack}>
      <div className={styles.group}>
        <label className={styles.toggle}>
          <input
            type="checkbox"
            checked={settings.enabled}
            onChange={(event) => {
              void change({ ...settings, enabled: event.target.checked });
            }}
          />
          {t('settings.presence.enabled')}
        </label>
      </div>
      <div className={styles.group}>
        <label className={styles.toggle}>
          <input
            type="checkbox"
            checked={settings.showNames}
            onChange={(event) => {
              void change({ ...settings, showNames: event.target.checked });
            }}
          />
          {t('settings.presence.showNames')}
        </label>
        <p className={styles.help}>{t('settings.presence.showNamesHelp')}</p>
      </div>
      <CollapsibleSection title={t('settings.presence.advanced')} defaultOpen={false}>
        <form
          className={styles.group}
          onSubmit={(event) => {
            event.preventDefault();
            void change({ ...settings, clientId: clientId.trim() === '' ? null : clientId.trim() });
          }}
        >
          <label htmlFor="settings-presence-client-id" className={styles.label}>
            {t('settings.presence.clientId')}
          </label>
          <div className={styles.inline}>
            <input
              id="settings-presence-client-id"
              className={styles.control}
              inputMode="numeric"
              value={clientId}
              placeholder={t('settings.presence.clientIdPlaceholder')}
              onChange={(event) => {
                setClientId(event.target.value);
              }}
            />
            <Button type="submit" variant="secondary">
              {t('settings.presence.clientIdSave')}
            </Button>
          </div>
          <p className={styles.help}>{t('settings.presence.clientIdHelp')}</p>
        </form>
      </CollapsibleSection>
      <CollapsibleSection title={t('settings.sandbox.how')} defaultOpen={false}>
        <p className={styles.help}>{t('settings.presence.help')}</p>
      </CollapsibleSection>
      {failure && <p role="alert">{failure}</p>}
    </div>
  );
}
