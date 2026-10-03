import { enUS } from './en-US';
import { appErrorMessage, errorMessage, failureMessage } from './messages';
import { createTranslator, DEFAULT_LANGUAGE, isLanguage, LANGUAGES } from './index';
import { ptBR } from './pt-BR';

describe('i18n', () => {
  it('defaults to Portuguese (Brazil) and supports English', () => {
    expect(DEFAULT_LANGUAGE).toBe('pt-BR');
    expect([...LANGUAGES]).toEqual(['pt-BR', 'en-US']);
    expect(isLanguage('en-US')).toBe(true);
    expect(isLanguage('fr-FR')).toBe(false);
  });

  it('translates the same key differently per language and fills placeholders', () => {
    const pt = createTranslator('pt-BR');
    const en = createTranslator('en-US');

    expect(pt('workspace.addAgent')).toBe('+ Adicionar agente');
    expect(en('workspace.addAgent')).toBe('+ Add Agent');
    expect(en('workspace.removeAgent', { name: 'Architect' })).toBe(
      'Remove Architect from workspace',
    );
    expect(pt('workspace.removeAgent', { name: 'Architect' })).toBe(
      'Remover Architect do workspace',
    );
  });

  it('leaves an unknown placeholder visible instead of printing "undefined"', () => {
    expect(createTranslator('en-US')('error.name_too_long')).toBe(
      'The name must be at most {max} characters.',
    );
  });

  it('has exactly the same keys in every language and no empty text', () => {
    expect(Object.keys(ptBR).sort()).toEqual(Object.keys(enUS).sort());
    for (const dictionary of [enUS, ptBR]) {
      expect(Object.values(dictionary).every((text) => text.trim() !== '')).toBe(true);
    }
  });

  it('words every core error code and failure kind in both languages', () => {
    const codes = Object.keys(enUS)
      .filter((key) => key.startsWith('error.') && key !== 'error.generic')
      .map((key) => key.slice('error.'.length));
    expect(codes.length).toBeGreaterThanOrEqual(20);
    const pt = createTranslator('pt-BR');
    const en = createTranslator('en-US');

    expect(
      appErrorMessage(pt, {
        code: 'project_folder_not_found',
        params: { path: '/x' },
        detail: null,
      }),
    ).toBe('A pasta “/x” não existe.');
    expect(
      appErrorMessage(en, {
        code: 'project_folder_not_found',
        params: { path: '/x' },
        detail: null,
      }),
    ).toBe('The folder “/x” does not exist.');
    expect(failureMessage(pt, 'authentication_required')).toContain('login');
    expect(failureMessage(en, 'authentication_required')).toContain('sign in');
    expect(failureMessage(en, 'something_new')).toBe('The request failed.');
  });

  it('shows a generic message, not raw text, for an error that is not coded', () => {
    const en = createTranslator('en-US');

    expect(errorMessage(en, new Error('ECONNRESET at 0x1f'))).toBe('Something went wrong.');
  });
});
