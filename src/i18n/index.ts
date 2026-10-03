import { enUS, type TranslationKey } from './en-US';
import { ptBR } from './pt-BR';

export type { TranslationKey } from './en-US';

/** Languages the interface is translated into. Adding one: a new dictionary file plus an entry here. */
export const LANGUAGES = ['pt-BR', 'en-US'] as const;
export type Language = (typeof LANGUAGES)[number];

export const DEFAULT_LANGUAGE: Language = 'pt-BR';

const DICTIONARIES: Record<Language, Record<TranslationKey, string>> = {
  'pt-BR': ptBR,
  'en-US': enUS,
};

export function isLanguage(value: string): value is Language {
  return (LANGUAGES as readonly string[]).includes(value);
}

export type TranslateParams = Record<string, string | number>;
export type Translate = (key: TranslationKey, params?: TranslateParams) => string;

/** Builds the `t` function of a language: looks the key up and fills `{name}` placeholders. */
export function createTranslator(language: Language): Translate {
  const dictionary = DICTIONARIES[language];
  return (key, params) =>
    dictionary[key].replace(/\{(\w+)\}/g, (placeholder, name: string) => {
      const value = params?.[name];
      return value === undefined ? placeholder : String(value);
    });
}
