import { useEffect, useState } from 'react';
import { Badge } from '@/components/ui/Badge';
import { CollapsibleSection } from '@/components/ui/CollapsibleSection';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import type { SandboxModeDto, SandboxSettingsDto, SandboxStatusDto } from '@/lib/tauri/commands';
import { getSandboxStatus, setSandboxSettings } from '../services/settingsService';
import styles from '../pages/SettingsPage.module.css';

const MODES: readonly SandboxModeDto[] = ['off', 'auto', 'required'];

/**
 * Whether the operating system confines the program of a runtime and what it starts (ADR 0038).
 * Says what this system can do, and never offers more than it can: where there is no sandbox the
 * panel says what each choice then means.
 */
export function SandboxSettings() {
  const { t } = useI18n();
  const [status, setStatus] = useState<SandboxStatusDto | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    Promise.resolve(getSandboxStatus())
      .then((loaded) => {
        if (!cancelled) setStatus(loaded);
      })
      .catch((error: unknown) => {
        if (!cancelled) setFailure(errorMessage(t, error));
      });
    return () => {
      cancelled = true;
    };
  }, [t]);

  async function change(next: SandboxSettingsDto) {
    setFailure(null);
    try {
      const saved = await setSandboxSettings(next);
      setStatus((current) => (current ? { ...current, settings: saved.sandbox ?? next } : current));
    } catch (error) {
      setFailure(errorMessage(t, error));
    }
  }

  if (!status) return failure ? <p role="alert">{failure}</p> : null;
  const { settings } = status;

  return (
    <div className={styles.stack}>
      <p className={styles.availability} data-available={status.available}>
        <Badge dot mono tone={status.available ? 'success' : 'warning'}>
          {t(status.available ? 'settings.sandbox.available' : 'settings.sandbox.unavailable')}
        </Badge>
      </p>
      <div className={styles.group}>
        <label htmlFor="settings-sandbox-mode" className={styles.label}>
          {t('settings.sandbox.mode')}
        </label>
        <select
          id="settings-sandbox-mode"
          className={styles.control}
          value={settings.mode}
          onChange={(event) => {
            const mode = MODES.find((m) => m === event.target.value);
            if (mode) void change({ ...settings, mode });
          }}
        >
          {MODES.map((mode) => (
            <option key={mode} value={mode}>
              {t(`settings.sandbox.mode.${mode}`)}
            </option>
          ))}
        </select>
      </div>
      <div className={styles.group}>
        <label className={styles.toggle}>
          <input
            type="checkbox"
            checked={settings.devCaches}
            onChange={(event) => {
              void change({ ...settings, devCaches: event.target.checked });
            }}
          />
          {t('settings.sandbox.devCaches')}
        </label>
        <p className={styles.help}>{t('settings.sandbox.devCachesHelp')}</p>
      </div>
      <CollapsibleSection title={t('settings.sandbox.how')} defaultOpen={false}>
        <p className={styles.help}>{t('settings.sandbox.help')}</p>
      </CollapsibleSection>
      {failure && <p role="alert">{failure}</p>}
    </div>
  );
}
