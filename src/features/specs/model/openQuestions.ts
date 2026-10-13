const HEADING = /^(#{1,6})\s*(open questions|questões em aberto|perguntas em aberto)\s*:?\s*$/i;
const ITEM = /^(?:[-*+]|\d+[.)])\s+(.*)$/;
const NOTHING = /^(none|n\/a|nenhuma?|nada|-|—)\.?$/i;

/**
 * What a document says it still does not know: the items under its "Open Questions" heading.
 * Read from the text, not asked of an agent, so it cannot be argued away: if the document lists
 * a question, the person is shown it.
 */
export function openQuestionsOf(content: string): string[] {
  const lines = content.split('\n');
  const start = lines.findIndex((line) => HEADING.test(line.trim()));
  if (start < 0) return [];
  const level = HEADING.exec(lines[start]?.trim() ?? '')?.[1]?.length ?? 2;
  const items: string[] = [];
  for (const raw of lines.slice(start + 1)) {
    const line = raw.trim();
    const heading = /^(#{1,6})\s/.exec(line);
    if (heading && (heading[1]?.length ?? 6) <= level) break;
    const item = ITEM.exec(raw.replace(/^\s{0,3}/, ''));
    if (item) {
      items.push(item[1] ?? '');
    } else if (line !== '' && items.length > 0 && /^\s/.test(raw)) {
      // A wrapped line of the item above.
      items.push(`${items.pop() ?? ''} ${line}`);
    }
  }
  return items.map((i) => i.replace(/\*\*|`/g, '').trim()).filter((i) => i && !NOTHING.test(i));
}
