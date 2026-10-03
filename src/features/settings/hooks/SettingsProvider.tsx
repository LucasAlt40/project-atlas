import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import { I18nProvider } from '@/i18n/I18nProvider';
import { DEFAULT_LANGUAGE, isLanguage, type Language } from '@/i18n';
import type { AppSettingsDto } from '@/lib/tauri/commands';
import { getSettings, selectWorkspace, setLanguage } from '../services/settingsService';

interface Settings {
  language: Language;
  /** The workspace that was open last (`null`: none yet). */
  selectedWorkspaceId: string | null;
  changeLanguage: (language: Language) => Promise<void>;
  rememberWorkspace: (workspaceId: string | null) => Promise<void>;
}

const SettingsContext = createContext<Settings | null>(null);

const DEFAULTS: AppSettingsDto = { language: DEFAULT_LANGUAGE, selectedWorkspaceId: null };

/**
 * Global application settings, persisted by the core, and the interface language derived from
 * them. Nothing is rendered until they are loaded, so the UI never flashes the wrong language.
 */
export function SettingsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<AppSettingsDto | null>(null);

  useEffect(() => {
    let cancelled = false;
    getSettings()
      .then((loaded) => {
        if (!cancelled) setSettings(loaded);
      })
      .catch(() => {
        // Without the core there is nothing to remember: start from the defaults.
        if (!cancelled) setSettings(DEFAULTS);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const changeLanguage = useCallback(async (language: Language) => {
    // The interface switches at once; the core remembers it for next time.
    setSettings((current) => (current ? { ...current, language } : current));
    setSettings(await setLanguage(language));
  }, []);

  const rememberWorkspace = useCallback(async (workspaceId: string | null) => {
    setSettings((current) =>
      current ? { ...current, selectedWorkspaceId: workspaceId } : current,
    );
    setSettings(await selectWorkspace(workspaceId));
  }, []);

  const language = settings && isLanguage(settings.language) ? settings.language : DEFAULT_LANGUAGE;
  const value = useMemo<Settings | null>(
    () =>
      settings && {
        language,
        selectedWorkspaceId: settings.selectedWorkspaceId,
        changeLanguage,
        rememberWorkspace,
      },
    [settings, language, changeLanguage, rememberWorkspace],
  );

  if (!value) return null;
  return (
    <SettingsContext.Provider value={value}>
      <I18nProvider language={language}>{children}</I18nProvider>
    </SettingsContext.Provider>
  );
}

export function useSettings(): Settings {
  const settings = useContext(SettingsContext);
  if (!settings) throw new Error('useSettings must be used inside <SettingsProvider>');
  return settings;
}
