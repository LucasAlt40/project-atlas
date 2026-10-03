import { createContext, useContext, useMemo, type ReactNode } from 'react';
import { createTranslator, type Language, type Translate } from './index';

interface I18n {
  language: Language;
  t: Translate;
}

/**
 * Outside a provider (in component tests) English is used. The app always mounts the provider,
 * whose language comes from the persisted settings (Portuguese by default).
 */
const I18nContext = createContext<I18n>({ language: 'en-US', t: createTranslator('en-US') });

export function I18nProvider({ language, children }: { language: Language; children: ReactNode }) {
  const value = useMemo(() => ({ language, t: createTranslator(language) }), [language]);
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18n {
  return useContext(I18nContext);
}

/** The translate function of the current language. */
export function useT(): Translate {
  return useContext(I18nContext).t;
}
