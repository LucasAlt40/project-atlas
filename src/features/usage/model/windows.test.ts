import { usageWindows } from './windows';

describe('usageWindows', () => {
  it('starts today at local midnight, the week on Monday and the month on the 1st', () => {
    // Wednesday 2026-10-14 15:30 local time.
    const now = new Date(2026, 9, 14, 15, 30);

    const windows = usageWindows(now);

    expect(new Date(windows.todayStart)).toEqual(new Date(2026, 9, 14, 0, 0));
    expect(new Date(windows.weekStart)).toEqual(new Date(2026, 9, 12, 0, 0));
    expect(new Date(windows.monthStart)).toEqual(new Date(2026, 9, 1, 0, 0));
  });

  it('treats Sunday as the last day of the week that began on the previous Monday', () => {
    const sunday = new Date(2026, 9, 18, 9, 0);

    expect(new Date(usageWindows(sunday).weekStart)).toEqual(new Date(2026, 9, 12, 0, 0));
  });

  it('crosses month boundaries', () => {
    const friday = new Date(2026, 10, 6, 12, 0); // Friday 6 Nov: the week began in October

    const windows = usageWindows(friday);

    expect(new Date(windows.weekStart)).toEqual(new Date(2026, 10, 2, 0, 0));
    expect(new Date(windows.monthStart)).toEqual(new Date(2026, 10, 1, 0, 0));
  });
});
