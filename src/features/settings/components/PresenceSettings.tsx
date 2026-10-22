import { useEffect, useState } from 'react';
import { Switch } from '@/components/ui/Switch';
import { CollapsibleSection } from '@/components/ui/CollapsibleSection';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import type { PresenceSettingsDto } from '@/lib/tauri/commands';
import { getSettings, setPresenceSettings } from '../services/settingsService';
import styles from '../pages/SettingsPage.module.css';

const OFF: PresenceSettingsDto = { enabled: false, showNames: true };

/**
 * Discord Rich Presence (proof of concept): what Atlas may show on the person's profile. Off until
 * switched on; the names are a separate choice. What is shown is composed by the core.
 */
export function PresenceSettings() {
  const { t } = useI18n();
  const [settings, setSettings] = useState<PresenceSettingsDto | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    Promise.resolve(getSettings())
      .then((loaded) => {
        if (cancelled) return;
        setSettings(loaded.presence ?? OFF);
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
      <h2 className={styles.heading}>{t('settings.presence.title')}</h2>
      <div className={styles.switchRow}>
        <Switch
          labelPosition="before"
          checked={settings.enabled}
          onChange={(enabled) => {
            void change({ ...settings, enabled });
          }}
        >
          {t('settings.presence.enabled')}
        </Switch>
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
      <CollapsibleSection
        title={t('settings.sandbox.how')}
        variant="disclosure"
        defaultOpen={false}
      >
        <p className={styles.help}>{t('settings.presence.help')}</p>
      </CollapsibleSection>
      {failure && <p role="alert">{failure}</p>}
    </div>
  );
}
