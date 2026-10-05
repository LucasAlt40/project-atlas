/**
 * A small syntax highlighter: enough to read code at a glance, not a parser. It knows comments,
 * strings, numbers and the keywords of a few language families, line by line (a block comment is
 * remembered across lines). It is deliberately not a dependency: the viewer is for following an
 * agent's work, not for editing.
 */

export type TokenKind = 'plain' | 'keyword' | 'string' | 'comment' | 'number';

export interface Token {
  text: string;
  kind: TokenKind;
}

export type Family = 'c' | 'hash' | 'json' | 'css' | 'plain';

const C_FAMILY = new Set(
  'ts tsx js jsx mjs cjs java rs go kt kts c h cpp hpp cc cs swift scala php dart'.split(' '),
);
const HASH_FAMILY = new Set('py rb sh bash zsh yml yaml toml ini conf'.split(' '));

export function familyOf(path: string): Family {
  const name = path.split('/').at(-1) ?? path;
  const ext = name.includes('.') ? (name.split('.').at(-1) ?? '').toLowerCase() : '';
  if (C_FAMILY.has(ext)) return 'c';
  if (HASH_FAMILY.has(ext)) return 'hash';
  if (ext === 'json') return 'json';
  if (ext === 'css' || ext === 'scss') return 'css';
  return 'plain';
}

const KEYWORDS = new Set(
  (
    'abstract as async await break case catch class const continue default defer do else enum ' +
    'export extends false final finally fn for from func function if impl implements import in ' +
    'instanceof interface let match mod mut new null nil override package private protected pub ' +
    'public readonly return self static struct super switch this throw throws trait true try type ' +
    'typeof undefined use val var void while with yield def elif except lambda None True False ' +
    'pass raise then fi esac done'
  ).split(' '),
);

export interface LineState {
  /** Inside a block comment that continues on the next line. */
  block: boolean;
}

export const START: LineState = { block: false };

export function highlightLine(
  line: string,
  family: Family,
  state: LineState = START,
): { tokens: Token[]; state: LineState } {
  if (family === 'plain') return { tokens: [{ text: line, kind: 'plain' }], state };
  const tokens: Token[] = [];
  let plain = '';
  const flush = () => {
    if (plain) tokens.push({ text: plain, kind: 'plain' });
    plain = '';
  };
  const push = (text: string, kind: TokenKind) => {
    flush();
    tokens.push({ text, kind });
  };
  let block = state.block;
  let i = 0;
  while (i < line.length) {
    const rest = line.slice(i);
    if (block) {
      const end = rest.indexOf('*/');
      if (end === -1) {
        push(rest, 'comment');
        i = line.length;
      } else {
        push(rest.slice(0, end + 2), 'comment');
        i += end + 2;
        block = false;
      }
      continue;
    }
    if (
      family !== 'json' &&
      ((family === 'hash' && rest.startsWith('#')) || rest.startsWith('//'))
    ) {
      if (family === 'css' && rest.startsWith('//')) {
        plain += rest[0] ?? '';
        i += 1;
        continue;
      }
      push(rest, 'comment');
      i = line.length;
      continue;
    }
    if ((family === 'c' || family === 'css') && rest.startsWith('/*')) {
      block = true;
      push('/*', 'comment');
      i += 2;
      continue;
    }
    const quote = rest[0];
    if (quote === '"' || quote === "'" || (quote === '`' && family === 'c')) {
      let j = 1;
      while (j < rest.length && rest[j] !== quote) j += rest[j] === '\\' ? 2 : 1;
      push(rest.slice(0, Math.min(j + 1, rest.length)), 'string');
      i += Math.min(j + 1, rest.length);
      continue;
    }
    const number = /^\d[\d_]*(\.\d+)?/.exec(rest);
    if (number && !/\w/.test(line[i - 1] ?? '')) {
      push(number[0], 'number');
      i += number[0].length;
      continue;
    }
    const word = /^[A-Za-z_$][\w$]*/.exec(rest);
    if (word) {
      if (family !== 'json' && KEYWORDS.has(word[0])) push(word[0], 'keyword');
      else if (family === 'json' && /^(true|false|null)$/.test(word[0])) push(word[0], 'keyword');
      else plain += word[0];
      i += word[0].length;
      continue;
    }
    plain += rest[0] ?? '';
    i += 1;
  }
  flush();
  return { tokens, state: { block } };
}
