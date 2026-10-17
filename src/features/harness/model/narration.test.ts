import { readNarration } from './narration';

describe('readNarration', () => {
  it('keeps what the agent said in words', () => {
    const n = readNarration('Vou ler o pom.xml.\n\nDepois as camadas.');
    expect(n.prose).toEqual(['Vou ler o pom.xml.', 'Depois as camadas.']);
    expect(n.findings).toEqual([]);
    expect(n.building).toBe(false);
  });

  it('turns the streamed JSON into the findings finished so far, never the code', () => {
    const raw =
      'Pronto.\n```json\n{"findings":[{"category":"module","key":"infra","value":"Implementa \\"tudo\\".","confidence":"medium","reason":"r","evidence":["a"]},' +
      '{"category":"convention","key":"naming","value":"Prefixo I","confidence":"medium","reason":"r","evidence":["b"]},' +
      '{"category":"module","key":"presentation","value":"Controllers REST (Admin, Au';

    const n = readNarration(raw);

    expect(n.prose).toEqual(['Pronto.']);
    expect(n.findings).toEqual([
      { category: 'module', key: 'infra', value: 'Implementa "tudo".' },
      { category: 'convention', key: 'naming', value: 'Prefixo I' },
    ]);
    expect(n.building).toBe(true);
  });

  it('says it is building when the object has begun but nothing is complete', () => {
    expect(readNarration('{"findings":[{"categ')).toEqual({
      prose: [],
      findings: [],
      building: true,
    });
  });
});
