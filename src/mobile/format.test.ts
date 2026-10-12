import { describe, expect, it } from 'vitest';
import { clock, deviceNameFromAgent, duration, pairingCodeFrom } from './format';
import { summary } from './fixtures';
import { alertsBetween } from './useRemote';

describe('format', () => {
  it('writes how long something took', () => {
    expect(duration(-5)).toBe('0s');
    expect(duration(42_000)).toBe('42s');
    expect(duration(192_000)).toBe('3m 12s');
    expect(duration(3_900_000)).toBe('1h 05m');
  });

  it('leaves out the date for a time of today', () => {
    const now = Date.UTC(2026, 9, 12, 15, 0);
    expect(clock(now, 'en-US', now)).not.toMatch(/Oct/);
    expect(clock(now - 3 * 86_400_000, 'en-US', now)).toMatch(/Oct/);
  });

  it('keeps only what can be in a pairing code', () => {
    expect(pairingCodeFrom('abcd-efgh ijkl_mnop!')).toBe('ABCDEFGHIJKLMNOP');
    expect(pairingCodeFrom('0189')).toBe('');
  });

  it('names the phone from what the browser says', () => {
    expect(deviceNameFromAgent('Mozilla/5.0 (iPhone; CPU iPhone OS 17_0)')).toBe('iPhone');
    expect(deviceNameFromAgent('Mozilla/5.0 (Linux; Android 14; Pixel 8) Mobile')).toBe('Pixel 8');
    expect(deviceNameFromAgent('Mozilla/5.0 (X11; Linux)')).toBe('Phone');
  });
});

describe('what is worth telling the person', () => {
  const before = (...runs: ReturnType<typeof summary>[]) => new Map(runs.map((r) => [r.id, r]));

  it('tells when a question starts waiting, and when another one does', () => {
    const quiet = summary();
    const asking = summary({ attention: 'needs_answer', pending: 1, status: 'waiting_for_input' });
    expect(alertsBetween(before(quiet), [asking], 1).map((a) => a.kind)).toEqual(['needs_answer']);
    const more = { ...asking, pending: 2 };
    expect(alertsBetween(before(asking), [more], 2).map((a) => a.kind)).toEqual(['needs_answer']);
    expect(alertsBetween(before(asking), [asking], 3)).toEqual([]);
  });

  it('tells when something needs the computer, a run fails, or a run finishes', () => {
    const running = summary();
    expect(
      alertsBetween(
        before(running),
        [summary({ attention: 'needs_computer', status: 'interrupted' })],
        1,
      ).map((a) => a.kind),
    ).toEqual(['needs_computer']);
    expect(
      alertsBetween(before(running), [summary({ status: 'failed', attention: 'failed' })], 1).map(
        (a) => a.kind,
      ),
    ).toEqual(['failed']);
    expect(
      alertsBetween(before(running), [summary({ status: 'completed' })], 1).map((a) => a.kind),
    ).toEqual(['completed']);
  });

  it('does not announce what was already finished when the page opened', () => {
    expect(alertsBetween(before(), [summary({ status: 'completed' })], 1)).toEqual([]);
  });
});
