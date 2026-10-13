import { contextFromAnswers, profileOf, questionsFor, topicsOf } from './contextQuestions';

const ids = (request: string, profile = profileOf('backend', [])) =>
  questionsFor(request, profile).map((q) => q.id);

describe('profileOf', () => {
  it('goes by the role the person gave, then by the technologies, and rules nothing out when it knows nothing', () => {
    expect(profileOf('frontend', ['Node.js'])).toEqual({ ui: true, server: false, data: false });
    expect(profileOf('library', [])).toEqual({ ui: false, server: false, data: false });
    expect(profileOf(null, ['React', 'Vite'])).toEqual({ ui: true, server: false, data: false });
    expect(profileOf(null, ['Express', 'Prisma'])).toEqual({ ui: false, server: true, data: true });
    expect(profileOf(null, [])).toEqual({ ui: true, server: true, data: true });
    expect(profileOf('other', ['Makefile'])).toEqual({ ui: true, server: true, data: true });
  });
});

describe('questionsFor', () => {
  it('always asks the few that apply to any change', () => {
    expect(ids('Algo simples e curto aqui')).toEqual(['goal', 'untouched', 'existing']);
  });

  it('asks about tables and stored data when the description is about data and the project has any', () => {
    const asked = ids('Excluir a conta do usuário e apagar os registros');
    expect(asked).toContain('data');
    expect(asked).toContain('dataMigration');
  });

  it('does not ask about tables for a project that only draws screens', () => {
    const front = profileOf('frontend', []);
    const asked = ids('Excluir a conta do usuário e apagar os registros', front);
    expect(asked).not.toContain('data');
    expect(asked).not.toContain('dataMigration');
  });

  it('asks a frontend which endpoints it calls, and a backend which systems it talks to', () => {
    const request = 'Cancelar a assinatura chamando o gateway de pagamento';
    expect(ids(request, profileOf('frontend', []))).toContain('contract');
    expect(ids(request, profileOf('frontend', []))).not.toContain('integration');
    expect(ids(request)).toContain('integration');
    expect(ids(request)).not.toContain('contract');
  });

  it('asks about screens only where there are screens, and about volume only where there is a server', () => {
    expect(ids('Nova tela com botão', profileOf('backend', []))).not.toContain('ui');
    expect(ids('Nova tela com botão', profileOf('frontend', []))).toContain('ui');
    expect(ids('Rodar em lote todo dia', profileOf('frontend', []))).not.toContain('volume');
    expect(ids('Rodar em lote todo dia')).toContain('volume');
  });

  it('reads English as well', () => {
    expect(topicsOf('Delete the account table rows and call the payment API')).toEqual(
      expect.arrayContaining(['data', 'integration']),
    );
  });
});

describe('contextFromAnswers', () => {
  it('keeps the free text and puts each answer under its question, leaving blanks out', () => {
    expect(
      contextFromAnswers(' See the wiki ', [
        { question: 'Which tables?', answer: 'users, subscriptions' },
        { question: 'Volume?', answer: '  ' },
      ]),
    ).toBe('See the wiki\n\nAnswers from the person:\n- Which tables?\n  users, subscriptions');
    expect(contextFromAnswers('', [])).toBe('');
  });
});
