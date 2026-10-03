import { AppInfoPanel } from '@/features/home/components/AppInfoPanel';
import { useAppInfo } from '@/features/home/hooks/useAppInfo';
import { LANGUAGES } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import { useSettings } from '../hooks/SettingsProvider';
import styles from './SettingsPage.module.css';

/** Global application settings. Workspace-specific settings live with their workspace. */
export function SettingsPage() {
  const { t, language } = useI18n();
  const { changeLanguage } = useSettings();
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

      <div className={styles.field}>
        <h2 className={styles.heading}>{t('settings.about')}</h2>
        <AppInfoPanel state={appInfo} />
      </div>
    </section>
  );
}
