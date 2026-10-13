import { useState } from 'react';
import { AppInfoPanel } from '@/features/home/components/AppInfoPanel';
import { useAppInfo } from '@/features/home/hooks/useAppInfo';
import { PortabilitySettings } from '@/features/portability';
import { LANGUAGES } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import { UpdateChecker } from '../components/UpdateChecker';
import { useSettings } from '../hooks/SettingsProvider';
import styles from './SettingsPage.module.css';

/** Global application settings. Workspace-specific settings live with their workspace. */
export function SettingsPage() {
  const { t, language } = useI18n();
  const { changeLanguage, specsMaxTokens, changeSpecsMaxTokens } = useSettings();
  const [budget, setBudget] = useState(specsMaxTokens === null ? '' : String(specsMaxTokens));
  const appInfo = useAppInfo();

  return (
    <section className={styles.page}>
      <h1 className={styles.title}>{t('settings.title')}</h1>

      <div className={styles.field}>
        <label htmlFor="settings-language" className={styles.label}>
          {t('settings.language')}
        </label>
        <select
          id="settings-language"
          className={styles.control}
          value={language}
          onChange={(e) => {
            const chosen = LANGUAGES.find((code) => code === e.target.value);
            if (chosen) void changeLanguage(chosen);
          }}
        >
          {LANGUAGES.map((code) => (
            <option key={code} value={code}>
              {t(`language.${code}`)}
            </option>
          ))}
        </select>
        <p className={styles.help}>{t('settings.languageHelp')}</p>
      </div>

      <form
        className={styles.field}
        onSubmit={(event) => {
          event.preventDefault();
          const parsed = Number(budget.replace(/[.,\s]/g, ''));
          void changeSpecsMaxTokens(
            budget.trim() === '' || !(parsed > 0) ? null : Math.floor(parsed),
          );
        }}
      >
        <label htmlFor="settings-specs-budget" className={styles.label}>
          {t('settings.specsBudget')}
        </label>
        <div className={styles.inline}>
          <input
            id="settings-specs-budget"
            className={styles.control}
            inputMode="numeric"
            value={budget}
            placeholder={t('settings.specsBudgetPlaceholder')}
            onChange={(event) => {
              setBudget(event.target.value);
            }}
          />
          <button type="submit" className={styles.control}>
            {t('settings.specsBudgetSave')}
          </button>
        </div>
        <p className={styles.help}>{t('settings.specsBudgetHelp')}</p>
      </form>

      <div className={styles.field}>
        <h2 className={styles.heading}>{t('portability.settings.title')}</h2>
        <PortabilitySettings />
      </div>

      <div className={styles.field}>
        <h2 className={styles.heading}>{t('settings.about')}</h2>
        <AppInfoPanel state={appInfo} />
      </div>

      <div className={styles.field}>
        <h2 className={styles.heading}>{t('settings.updates')}</h2>
        <UpdateChecker />
      </div>
    </section>
  );
}
