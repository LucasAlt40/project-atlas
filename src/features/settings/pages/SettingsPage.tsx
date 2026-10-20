import { useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { AppInfoPanel } from '@/features/home/components/AppInfoPanel';
import { useAppInfo } from '@/features/home/hooks/useAppInfo';
import { MemorySettings } from '@/features/memory';
import { PortabilitySettings } from '@/features/portability';
import { LANGUAGES } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import { ExternalMemorySettings } from '../components/ExternalMemorySettings';
import { PresenceSettings } from '../components/PresenceSettings';
import { SandboxSettings } from '../components/SandboxSettings';
import { UpdateChecker } from '../components/UpdateChecker';
import { useSettings } from '../hooks/SettingsProvider';
import styles from './SettingsPage.module.css';

const SECTIONS = ['general', 'memory', 'portability', 'about'] as const;
type Section = (typeof SECTIONS)[number];

/**
 * Global application settings, as one page with an index that goes straight to each section.
 * Workspace-specific settings live with their workspace.
 */
export function SettingsPage() {
  const { t, language } = useI18n();
  const { changeLanguage, specsMaxTokens, changeSpecsMaxTokens } = useSettings();
  const [budget, setBudget] = useState(specsMaxTokens === null ? '' : String(specsMaxTokens));
  const appInfo = useAppInfo();
  const [current, setCurrent] = useState<Section>('general');
  const content = useRef<HTMLDivElement>(null);

  // The index marks the section that is at the top of the page while it is scrolled.
  useEffect(() => {
    const root = content.current;
    if (!root || typeof IntersectionObserver === 'undefined') return undefined;
    const scroller = root.closest('main');
    const observer = new IntersectionObserver(
      (entries) => {
        const visible = entries
          .filter((entry) => entry.isIntersecting)
          .sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top)[0];
        const id = SECTIONS.find((section) => `settings-${section}` === visible?.target.id);
        if (id) setCurrent(id);
      },
      { root: scroller, rootMargin: '0px 0px -70% 0px' },
    );
    for (const section of SECTIONS) {
      const element = document.getElementById(`settings-${section}`);
      if (element) observer.observe(element);
    }
    return () => {
      observer.disconnect();
    };
  }, []);

  function goTo(section: Section) {
    setCurrent(section);
    const target = document.getElementById(`settings-${section}`);
    const scroller = target?.closest('main');
    if (!target || !scroller) return;
    // Only the content area moves; the window and its bars stay where they are.
    scroller.scrollTo({
      top:
        scroller.scrollTop +
        target.getBoundingClientRect().top -
        scroller.getBoundingClientRect().top -
        16,
    });
  }

  return (
    <section className={styles.page}>
      <div className={styles.layout}>
        <nav className={styles.index} aria-label={t('settings.nav.label')}>
          {SECTIONS.map((section) => (
            <button
              key={section}
              type="button"
              className={styles.indexItem}
              aria-current={current === section ? 'location' : undefined}
              onClick={() => {
                goTo(section);
              }}
            >
              {t(`settings.nav.${section}`)}
            </button>
          ))}
        </nav>

        <div className={styles.content} ref={content}>
          <h1 className={styles.title}>{t('settings.title')}</h1>

          <section
            id="settings-general"
            className={styles.section}
            aria-label={t('settings.nav.general')}
          >
            <div className={styles.group}>
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
              className={styles.group}
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
                <Button type="submit" variant="secondary">
                  {t('settings.specsBudgetSave')}
                </Button>
              </div>
              <p className={styles.help}>{t('settings.specsBudgetHelp')}</p>
            </form>

            <div className={styles.group}>
              <h2 className={styles.heading}>{t('settings.sandbox.title')}</h2>
              <SandboxSettings />
            </div>

            <div className={styles.group}>
              <h2 className={styles.heading}>{t('settings.presence.title')}</h2>
              <PresenceSettings />
            </div>
          </section>

          <section
            id="settings-memory"
            className={styles.section}
            aria-label={t('settings.nav.memory')}
          >
            <h2 className={styles.sectionTitle}>{t('settings.nav.memory')}</h2>
            <MemorySettings />
            <div className={styles.group}>
              <h2 className={styles.heading}>{t('settings.externalMemory.title')}</h2>
              <ExternalMemorySettings />
            </div>
          </section>

          <section
            id="settings-portability"
            className={styles.section}
            aria-label={t('settings.nav.portability')}
          >
            <h2 className={styles.sectionTitle}>{t('settings.nav.portability')}</h2>
            <PortabilitySettings />
          </section>

          <section
            id="settings-about"
            className={styles.section}
            aria-label={t('settings.nav.about')}
          >
            <h2 className={styles.sectionTitle}>{t('settings.nav.about')}</h2>
            <div className={styles.card}>
              <h3 className={styles.heading}>{t('settings.about')}</h3>
              <AppInfoPanel state={appInfo} />
              <h3 className={styles.heading}>{t('settings.updates')}</h3>
              <UpdateChecker />
            </div>
          </section>
        </div>
      </div>
    </section>
  );
}
