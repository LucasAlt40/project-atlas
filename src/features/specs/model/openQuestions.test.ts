import { openQuestionsOf } from './openQuestions';

describe('openQuestionsOf', () => {
  it('lists what a document says is still open, and stops at the next section of its level', () => {
    const text = [
      '# Design',
      '## Risks',
      '- not this',
      '## Open Questions',
      '- **Retention** of old logs: purge or mask?',
      '  The answer does not change the specs.',
      '- Timeout of the call.',
      '',
      '## Migration',
      '- not this either',
    ].join('\n');

    expect(openQuestionsOf(text)).toEqual([
      'Retention of old logs: purge or mask? The answer does not change the specs.',
      'Timeout of the call.',
    ]);
  });

  it('reads the Portuguese heading and numbered items', () => {
    expect(openQuestionsOf('## Questões em aberto\n1. Qual o prazo?\n2) Quem decide?')).toEqual([
      'Qual o prazo?',
      'Quem decide?',
    ]);
  });

  it('keeps identifiers with underscores as they were written', () => {
    expect(
      openQuestionsOf('## Open Questions\n- Who reads `gateway_cancel_pending` in logs_pagbank?'),
    ).toEqual(['Who reads gateway_cancel_pending in logs_pagbank?']);
  });

  it('finds nothing when there is no section, or it says there is nothing', () => {
    expect(openQuestionsOf('# Design\n\nText')).toEqual([]);
    expect(openQuestionsOf('## Open Questions\n- None')).toEqual([]);
    expect(openQuestionsOf('## Open Questions\n\n')).toEqual([]);
  });
});
