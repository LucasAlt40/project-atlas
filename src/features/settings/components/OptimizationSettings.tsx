import { useEffect, useState } from 'react';
import type { TranslationKey } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import type { OptimizationSwitchesDto } from '@/lib/tauri/commands';
import { getSettings, setOptimizationSwitches } from '../services/settingsService';
import styles from '../pages/SettingsPage.module.css';

const SWITCHES: readonly (keyof OptimizationSwitchesDto)[] = [
  'contextEnabled',
  'skillsEnabled',
  'guardrailsEnabled',
  'metricsEnabled',
];

/**
 * What makes each run cheaper or safer (ADR 0042). Everything here is on by default; the person
 * turns a part off, and that choice is kept.
 */
export function OptimizationSettings() {
  const { t } = useI18n();
  const [switches, setSwitches] = useState<OptimizationSwitchesDto | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    Promise.resolve(getSettings())
      .then((settings) => {
        const o = settings.optimization;
        if (!cancelled)
          setSwitches({
            metricsEnabled: o?.metricsEnabled ?? true,
            contextEnabled: o?.contextEnabled ?? true,
            skillsEnabled: o?.skillsEnabled ?? true,
            guardrailsEnabled: o?.guardrailsEnabled ?? true,
          });
      })
      .catch((error: unknown) => {
        if (!cancelled) setFailure(errorMessage(t, error));
      });
    return () => {
      cancelled = true;
    };
  }, [t]);

  async function change(next: OptimizationSwitchesDto) {
    setFailure(null);
    const before = switches;
    setSwitches(next);
    try {
      await setOptimizationSwitches(next);
    } catch (error) {
      setSwitches(before);
      setFailure(errorMessage(t, error));
    }
  }

  if (!switches) return failure ? <p role="alert">{failure}</p> : null;

  return (
    <div className={styles.stack}>
      <p className={styles.help}>{t('settings.optimization.help')}</p>
      {SWITCHES.map((key) => (
        <div key={key} className={styles.group}>
          <label className={styles.toggle}>
            <input
              type="checkbox"
              checked={switches[key]}
              onChange={(event) => {
                void change({ ...switches, [key]: event.target.checked });
              }}
            />
            {t(`settings.optimization.${key}` as TranslationKey)}
          </label>
          <p className={styles.help}>{t(`settings.optimization.${key}Help` as TranslationKey)}</p>
        </div>
      ))}
      {failure && <p role="alert">{failure}</p>}
    </div>
  );
}
