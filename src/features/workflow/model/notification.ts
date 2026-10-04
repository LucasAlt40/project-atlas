/** Longest plan excerpt put in a system notification (they cut long text anyway). */
const MAX_EXCERPT = 280;

/**
 * The start of a plan, as plain text for a system notification: Markdown marks, code and links
 * removed, lines joined with " · ", cut on a word. The notification is a pointer to the plan, not
 * the plan: the whole of it is in the panel Atlas opens.
 */
export function planExcerpt(document: string, max = MAX_EXCERPT): string {
  const lines = document
    .replace(/```[\s\S]*?```/g, ' ')
    .split('\n')
    .map((line) =>
      line
        .replace(/^\s{0,3}#{1,6}\s*/, '')
        .replace(/^\s*(?:[-*+]|\d+[.)])\s+/, '')
        .replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1')
        .replace(/[*_`>|]/g, '')
        .trim(),
    )
    .filter((line) => line !== '' && !/^[-=\s]+$/.test(line));
  const text = lines.join(' · ');
  if (text.length <= max) return text;
  const cut = text.slice(0, max);
  const space = cut.lastIndexOf(' ');
  return `${cut.slice(0, space > max / 2 ? space : max).trimEnd()}…`;
}
