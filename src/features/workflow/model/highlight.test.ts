import { familyOf, highlightLine, START } from './highlight';

const kinds = (line: string, family = familyOf('a.ts')) =>
  highlightLine(line, family)
    .tokens.filter((t) => t.kind !== 'plain')
    .map((t) => [t.kind, t.text]);

describe('syntax highlighting', () => {
  it('tells the family of a language from the file name', () => {
    expect(familyOf('src/a.ts')).toBe('c');
    expect(familyOf('A.java')).toBe('c');
    expect(familyOf('script.py')).toBe('hash');
    expect(familyOf('package.json')).toBe('json');
    expect(familyOf('LICENSE')).toBe('plain');
  });

  it('colors keywords, strings, numbers and comments', () => {
    expect(kinds('const x = "a b" + 42; // done')).toEqual([
      ['keyword', 'const'],
      ['string', '"a b"'],
      ['number', '42'],
      ['comment', '// done'],
    ]);
  });

  it('keeps a word that only contains a keyword as plain text', () => {
    expect(kinds('constant = nullable')).toEqual([]);
  });

  it('remembers a block comment across lines', () => {
    const first = highlightLine('/* start', familyOf('a.ts'), START);
    const second = highlightLine('still inside */ let y', familyOf('a.ts'), first.state);

    expect(first.tokens[0]).toMatchObject({ kind: 'comment' });
    expect(second.tokens.map((t) => [t.kind, t.text.trim()]).filter((t) => t[1])).toEqual([
      ['comment', 'still inside */'],
      ['keyword', 'let'],
      ['plain', 'y'],
    ]);
  });

  it('does not lose or add any text', () => {
    const line = 'import { a } from "./a"; // x \\ "quote';
    const text = highlightLine(line, 'c')
      .tokens.map((t) => t.text)
      .join('');

    expect(text).toBe(line);
  });

  it('leaves unknown files as they are', () => {
    expect(highlightLine('const x = 1', 'plain').tokens).toEqual([
      { text: 'const x = 1', kind: 'plain' },
    ]);
  });
});
