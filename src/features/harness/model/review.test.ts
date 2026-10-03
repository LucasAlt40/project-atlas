import { harnessSummary, projectAnalysis } from '@/test/fixtures';
import {
  architectureFindings,
  buildInput,
  detectedStack,
  initialReview,
  isEditableValue,
  stackFindings,
} from './review';

function required<T>(value: T | undefined): T {
  if (value === undefined) throw new Error('expected a value');
  return value;
}

describe('project review model', () => {
  it('splits findings into stack, architecture and repository facts', () => {
    const analysis = projectAnalysis();

    expect(detectedStack(analysis).map((f) => f.label)).toEqual([
      'TypeScript',
      'Angular 21',
      'Vitest',
      'Docker',
      'GitHub Actions',
    ]);
    // Reviewed in a fixed order: language first, CI last.
    expect(stackFindings(analysis)[0]?.id).toBe('language:typescript');
    expect(architectureFindings(analysis).map((f) => f.key)).toEqual(['layered']);
  });

  it('does not treat "unknown" as an architecture', () => {
    const analysis = projectAnalysis({
      findings: [
        {
          id: 'architecture:unknown',
          category: 'architecture',
          key: 'unknown',
          label: 'Unknown',
          value: 'unknown',
          confidence: 'low',
          origin: 'generated',
          evidence: [{ source: 'directory structure' }],
          byModel: false,
        },
      ],
    });

    expect(architectureFindings(analysis)).toEqual([]);
  });

  it('starts with everything ticked except weak architecture guesses', () => {
    const analysis = projectAnalysis();
    analysis.findings.push({
      ...required(analysis.findings.at(-1)),
      id: 'architecture:feature_based',
      key: 'feature_based',
      confidence: 'low',
    });

    expect(initialReview(analysis).excluded).toEqual(['architecture:feature_based']);
  });

  it('starts from last time’s choices when there were any', () => {
    const analysis = projectAnalysis({
      previousExcluded: ['testing:vitest'],
      previousCorrections: { 'framework:angular': '19' },
    });

    const review = initialReview(analysis);

    expect(review.excluded).toEqual(['testing:vitest']);
    expect(review.values).toEqual({ 'framework:angular': '19' });
  });

  it('sends only real corrections and only for findings that exist', () => {
    const analysis = projectAnalysis();
    const review = {
      ...initialReview(analysis),
      excluded: ['testing:vitest', 'ghost:finding'],
      values: {
        'framework:angular': ' 19 ',
        'testing:vitest': '3', // unchanged
        'language:typescript': '  ', // emptied: not a correction
        'ghost:finding': 'x',
      },
      purpose: ' ERP ',
    };

    expect(buildInput(analysis, review, 'create')).toEqual({
      mode: 'create',
      excluded: ['testing:vitest'],
      corrections: { 'framework:angular': '19' },
      confirmed: [],
      purpose: 'ERP',
      users: '',
      concepts: '',
      businessRules: '',
      constraints: '',
      decisions: '',
    });
  });

  it('only offers a value to edit when there is a real value', () => {
    const analysis = projectAnalysis();
    const byId = (id: string) => required(analysis.findings.find((f) => f.id === id));

    expect(isEditableValue(byId('framework:angular'))).toBe(true);
    expect(isEditableValue(byId('infrastructure:docker'))).toBe(false);
    expect(isEditableValue(byId('architecture:layered'))).toBe(false);
    expect(harnessSummary().status).toBe('not_initialized');
  });
});
