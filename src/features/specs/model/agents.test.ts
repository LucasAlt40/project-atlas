import { describe, expect, it } from 'vitest';
import { agent } from '@/test/fixtures';
import type { Translate } from '@/i18n';
import { authorName, suggestedAuthor } from './agents';

function withPersonality(id: string, name: string, personalityId: string) {
  return { ...agent(id, name), personalityId };
}

describe('suggestedAuthor', () => {
  it('prefers a spec architect, then an architect, then whoever is first', () => {
    const dev = withPersonality('d', 'Dev', 'developer');
    const architect = withPersonality('a', 'Arch', 'architect');
    const spec = withPersonality('s', 'Spec', 'spec-architect');

    expect(suggestedAuthor([dev, architect, spec])).toBe(spec);
    expect(suggestedAuthor([dev, architect])).toBe(architect);
    expect(suggestedAuthor([dev])).toBe(dev);
    expect(suggestedAuthor([])).toBeUndefined();
  });
});

describe('authorName', () => {
  const t = ((key: string, params?: Record<string, string>) =>
    `${key}:${params?.name ?? ''}`) as unknown as Translate;

  it('marks only the recommended authors', () => {
    expect(authorName(t, withPersonality('s', 'Spec', 'spec-architect'))).toBe(
      'specs.new.recommended:Spec',
    );
    expect(authorName(t, withPersonality('a', 'Arch', 'architect'))).toBe('Arch');
  });
});
