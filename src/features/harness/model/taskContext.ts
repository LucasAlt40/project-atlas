import type { TranslationKey, TranslateParams } from '@/i18n';
import type { ContextEntryDto, ContextAreaDto, TaskContextDto } from '@/lib/tauri/commands';

/** One line of "why": a translation key and what fills it. */
export interface ReasonLine {
  key: TranslationKey;
  params?: TranslateParams;
}

/** How much smaller than the whole Harness context this one is, as a whole percent (never < 0). */
export function reductionPercent(context: TaskContextDto): number {
  if (context.fullHarnessChars <= 0 || context.selectedChars >= context.fullHarnessChars) return 0;
  return Math.round((1 - context.selectedChars / context.fullHarnessChars) * 100);
}

export function areaKey(area: ContextAreaDto): TranslationKey {
  return `taskContext.area.${area}` as TranslationKey;
}

/** Constraints and decisions are told as one group: they are in every context. */
export function isRule(entry: ContextEntryDto): boolean {
  return entry.item.kind === 'constraint' || entry.item.kind === 'decision';
}

export function included(context: TaskContextDto): ContextEntryDto[] {
  return context.entries.filter((e) => e.outcome === 'included');
}

export function leftOut(context: TaskContextDto): ContextEntryDto[] {
  return context.entries.filter((e) => e.outcome !== 'included');
}

/** Why an item is in the context: every match that scored, in the order they weigh. */
export function includedBecause(entry: ContextEntryDto): ReasonLine[] {
  const r = entry.reason;
  const lines: ReasonLine[] = [];
  if (r.alwaysIncluded === 'constraint') lines.push({ key: 'taskContext.reason.constraint' });
  if (r.alwaysIncluded === 'decision') lines.push({ key: 'taskContext.reason.decision' });
  for (const tag of r.matchedTags) lines.push({ key: 'taskContext.reason.tag', params: { tag } });
  for (const area of r.matchedAreas)
    lines.push({ key: 'taskContext.reason.area', params: { area } });
  for (const category of r.matchedCategories)
    lines.push({ key: 'taskContext.reason.category', params: { category } });
  for (const path of r.matchedPaths)
    lines.push({ key: 'taskContext.reason.path', params: { path } });
  for (const word of r.matchedKeywords)
    lines.push({ key: 'taskContext.reason.keyword', params: { word } });
  return lines;
}

/** Why an item was left out. */
export function leftOutBecause(entry: ContextEntryDto): ReasonLine[] {
  const lines: ReasonLine[] = [
    {
      key:
        entry.outcome === 'over_budget'
          ? 'taskContext.reason.over_budget'
          : 'taskContext.reason.not_relevant',
    },
  ];
  for (const penalty of entry.reason.penalties)
    lines.push({ key: `taskContext.reason.${penalty}` as TranslationKey });
  return lines;
}
