import { useEffect, useState } from 'react';
import { Badge } from '@/components/ui/Badge';
import { CollapsibleSection } from '@/components/ui/CollapsibleSection';
import { SegmentedControl } from '@/components/ui/SegmentedControl';
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
      <div className={styles.headRow}>
        <h2 className={styles.heading}>{t('settings.sandbox.title')}</h2>
        <Badge dot tone={status.available ? 'success' : 'warning'}>
          {t(status.available ? 'settings.sandbox.available' : 'settings.sandbox.unavailable')}
        </Badge>
      </div>
      <SegmentedControl
        label={t('settings.sandbox.mode')}
        value={settings.mode}
        onChange={(mode) => {
          void change({ ...settings, mode });
        }}
        segments={MODES.map((mode) => ({
          value: mode,
          label: t(`settings.sandbox.mode.${mode}`),
        }))}
      />
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
      <CollapsibleSection
        title={t('settings.sandbox.how')}
        variant="disclosure"
        defaultOpen={false}
      >
        <p className={styles.help}>{t('settings.sandbox.help')}</p>
      </CollapsibleSection>
      {failure && <p role="alert">{failure}</p>}
    </div>
  );
}
