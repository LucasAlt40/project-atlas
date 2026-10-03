import type { UsageWindowsDto } from '@/lib/tauri/commands';

/**
 * Where "today", "this week" (Monday to Sunday) and "this month" start in the user's local
 * time zone. The core only sums records between these instants; the calendar is the UI's.
 */
export function usageWindows(now: Date = new Date()): UsageWindowsDto {
  const todayStart = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const daysSinceMonday = (todayStart.getDay() + 6) % 7;
  const weekStart = new Date(
    todayStart.getFullYear(),
    todayStart.getMonth(),
    todayStart.getDate() - daysSinceMonday,
  );
  const monthStart = new Date(now.getFullYear(), now.getMonth(), 1);
  return {
    todayStart: todayStart.getTime(),
    weekStart: weekStart.getTime(),
    monthStart: monthStart.getTime(),
  };
}
