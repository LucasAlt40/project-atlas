import { formatCost, formatDuration, formatReset, formatTokens } from './format';

describe('usage formatting', () => {
  it('abbreviates token counts in the interface language', () => {
    expect(formatTokens(12_400, 'en-US')).toBe('12.4K');
    expect(formatTokens(12_400, 'pt-BR')).toContain('12,4');
    expect(formatTokens(0, 'en-US')).toBe('0');
  });

  it('shows a cost with its currency only when the runtime gave one', () => {
    expect(formatCost(0.18, 'USD', 'en-US')).toBe('$0.18');
    expect(formatCost(0.18, null, 'en-US')).toBe('0.18');
    expect(formatCost(0, 'USD', 'en-US')).toBe('$0.00');
  });

  it('keeps small amounts visible', () => {
    expect(formatCost(0.0155, 'USD', 'en-US')).toBe('$0.0155');
  });

  it('formats durations compactly', () => {
    expect(formatDuration(12_400)).toBe('12s');
    expect(formatDuration(65_000)).toBe('1m 05s');
    expect(formatDuration(3_720_000)).toBe('1h 02m');
    expect(formatDuration(-5)).toBe('0s');
  });

  it('describes when a quota window resets', () => {
    const now = Date.UTC(2026, 9, 3, 12, 0, 0);

    expect(formatReset((now + 2 * 3_600_000) / 1000, now, 'en-US')).toBe('in 2 hours');
    expect(formatReset((now + 30 * 60_000) / 1000, now, 'en-US')).toBe('in 30 minutes');
  });
});
