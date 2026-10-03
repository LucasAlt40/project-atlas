import { LANGUAGES, type Language } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import { useSettings } from '../hooks/SettingsProvider';
import styles from './LanguageSwitch.module.css';

const SHORT: Record<Language, string> = { 'pt-BR': 'PT', 'en-US': 'EN' };

/** A compact language toggle for the header, so switching language is easy to find. */
export function LanguageSwitch() {
  const { t, language } = useI18n();
  const { changeLanguage } = useSettings();
  return (
    <div className={styles.switch} role="group" aria-label={t('language.switch')}>
      {LANGUAGES.map((code) => (
        <button
          key={code}
          type="button"
          className={styles.option}
          aria-pressed={code === language}
          title={t(`language.${code}`)}
          onClick={() => {
            void changeLanguage(code);
          }}
        >
          {SHORT[code]}
        </button>
      ))}
    </div>
  );
}
