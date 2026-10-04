import { planExcerpt } from './notification';

describe('planExcerpt', () => {
  it('turns the start of a Markdown plan into plain text', () => {
    const plan =
      '## Plan\n\n1. Add the **column**\n2. Fix [the webhook](http://x)\n\n```sql\nselect 1\n```\n';

    expect(planExcerpt(plan)).toBe('Plan · Add the column · Fix the webhook');
  });

  it('cuts a long plan on a word and says it goes on', () => {
    const excerpt = planExcerpt('word '.repeat(200), 50);

    expect(excerpt.length).toBeLessThanOrEqual(51);
    expect(excerpt.endsWith('…')).toBe(true);
  });

  it('is empty for nothing', () => {
    expect(planExcerpt('')).toBe('');
  });
});
