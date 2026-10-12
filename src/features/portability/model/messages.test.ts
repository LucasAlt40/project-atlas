import { createTranslator } from '@/i18n';
import { issueText, portabilityError } from './messages';

const en = createTranslator('en-US');
const pt = createTranslator('pt-BR');

describe('portability messages', () => {
  it('words a refusal by what the person should do, in both languages', () => {
    const error = {
      code: 'import_rejected',
      params: { reason: 'decision_required', item: 'Architect' },
      detail: null,
    };

    expect(portabilityError(en, error)).toBe(
      'Nothing was imported: decide what to do with “Architect”, which matches something you have.',
    );
    expect(portabilityError(pt, error)).toContain('Architect');
    expect(portabilityError(pt, error)).toMatch(/^Nada foi importado/);
  });

  it('words a rule an item breaks with the rule, not with a code', () => {
    expect(
      portabilityError(en, {
        code: 'import_rejected',
        params: { reason: 'runtime_unknown', item: 'Architect' },
        detail: null,
      }),
    ).toBe(
      'Nothing was imported: “Architect” cannot be created because its runtime does not exist in this Atlas.',
    );
  });

  it('falls back to the generic words for any other failure', () => {
    expect(portabilityError(en, new Error('boom'))).toBe(en('error.generic'));
    expect(portabilityError(en, { code: 'import_not_atlas_file', params: {}, detail: null })).toBe(
      'This is not a file exported by Atlas.',
    );
  });

  it('words an issue with its values, and shows the code when a newer core raises one it does not know', () => {
    expect(issueText(en, { code: 'invalid_spec', params: { reason: 'model_invalid' } })).toBe(
      'It cannot be imported: its model id is not acceptable.',
    );
    expect(issueText(en, { code: 'something_new', params: {} })).toBe('something_new');
  });
});
