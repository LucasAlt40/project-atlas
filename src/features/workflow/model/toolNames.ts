/**
 * Whether an MCP tool's name says it only reads. Mirrors the core's rule for the tools a template
 * suggests to a step that only consults (`application/workflow/template_tools.rs`): the first word
 * of the name is a reading verb. A guess from the name, used only to *narrow* what is granted.
 */
const READING = new Set([
  'get',
  'list',
  'search',
  'read',
  'fetch',
  'find',
  'query',
  'view',
  'describe',
  'inspect',
  'lookup',
  'resolve',
]);

function firstWord(tool: string): string {
  let word = '';
  for (const c of tool) {
    const upper = c !== c.toLowerCase();
    if (c === '_' || c === '-' || c === '.' || c === ' ' || (upper && word !== '')) break;
    word += c.toLowerCase();
  }
  return word;
}

export function readsOnly(tool: string): boolean {
  return READING.has(firstWord(tool));
}
