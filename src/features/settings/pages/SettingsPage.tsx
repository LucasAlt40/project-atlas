import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { SectionNav } from '@/components/ui/SectionNav';
import { AppInfoPanel } from '@/features/home/components/AppInfoPanel';
import { useAppInfo } from '@/features/home/hooks/useAppInfo';
import { MemorySettings } from '@/features/memory';
import { PortabilitySettings } from '@/features/portability';
import { LANGUAGES } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import { PresenceSettings } from '../components/PresenceSettings';
import { SandboxSettings } from '../components/SandboxSettings';
import { StorageSettings } from '../components/StorageSettings';
import { UpdateChecker } from '../components/UpdateChecker';
import { useSettings } from '../hooks/SettingsProvider';
import styles from './SettingsPage.module.css';

const SECTIONS = ['general', 'memory', 'storage', 'portability', 'about'] as const;
type Section = (typeof SECTIONS)[number];

/**
 * Global application settings: an index of sections on the left, one section at a time on the
 * right. Workspace-specific settings live with their workspace.
 */
export function SettingsPage() {
  const { t, language } = useI18n();
  const { changeLanguage, specsMaxTokens, changeSpecsMaxTokens } = useSettings();
  const [budget, setBudget] = useState(specsMaxTokens === null ? '' : String(specsMaxTokens));
  const appInfo = useAppInfo();
  const [current, setCurrent] = useState<Section>('general');

  return (
    <section className={styles.page}>
      <h1 className={styles.srOnly}>{t('settings.title')}</h1>
      <div className={styles.layout}>
        <SectionNav
          label={t('settings.nav.label')}
          active={current}
          onSelect={setCurrent}
          items={SECTIONS.map((section) => ({ id: section, label: t(`settings.nav.${section}`) }))}
        />

        <div className={styles.content}>
          <h2 className={styles.title}>{t(`settings.nav.${current}`)}</h2>

          {current === 'general' && (
            <section className={styles.section} aria-label={t('settings.nav.general')}>
              <div className={styles.group}>
                <label htmlFor="settings-language" className={styles.heading}>
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
                className={styles.group}
                onSubmit={(event) => {
                  event.preventDefault();
                  const parsed = Number(budget.replace(/[.,\s]/g, ''));
                  void changeSpecsMaxTokens(
                    budget.trim() === '' || !(parsed > 0) ? null : Math.floor(parsed),
                  );
                }}
              >
                <label htmlFor="settings-specs-budget" className={styles.heading}>
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
                  <Button type="submit" variant="secondary">
                    {t('settings.specsBudgetSave')}
                  </Button>
                </div>
                <p className={styles.help}>{t('settings.specsBudgetHelp')}</p>
              </form>

              <div className={styles.group}>
                <SandboxSettings />
              </div>

              <div className={styles.group}>
                <PresenceSettings />
              </div>
            </section>
          )}

          {current === 'memory' && (
            <section className={styles.section} aria-label={t('settings.nav.memory')}>
              <MemorySettings />
            </section>
          )}

          {current === 'storage' && (
            <section className={styles.section} aria-label={t('settings.nav.storage')}>
              <StorageSettings />
            </section>
          )}

          {current === 'portability' && (
            <section className={styles.section} aria-label={t('settings.nav.portability')}>
              <PortabilitySettings />
            </section>
          )}

          {current === 'about' && (
            <section className={styles.section} aria-label={t('settings.nav.about')}>
              <div className={styles.group}>
                <h3 className={styles.heading}>{t('settings.about')}</h3>
                <AppInfoPanel state={appInfo} />
              </div>
              <div className={styles.group}>
                <h3 className={styles.heading}>{t('settings.updates')}</h3>
                <UpdateChecker />
              </div>
            </section>
          )}
        </div>
      </div>
    </section>
  );
}
