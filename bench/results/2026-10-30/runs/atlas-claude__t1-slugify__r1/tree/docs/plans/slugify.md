# Plan: `slugify` module

Size: moderate

## Goals

Add a dependency-free CommonJS helper `slugify(text, options)` that turns arbitrary text into a URL-safe slug, with an optional `maxLength` that cuts on word boundaries, plus `node:test` tests.

## Scope

- In: `src/slugify.js` (new), `test/slugify.test.js` (new).
- Out: `package.json`, `README.md`, any other file, any dependency, any change to the existing `node --test` setup.
- Out (decided): non-ASCII letters with no ASCII decomposition (e.g. `ß`, `ø`, `ł`, CJK) are not transliterated. They are removed like any other non-`a-z0-9` character and act as separators. Transliteration tables are an abstraction the task does not need.

## Project context

- `package.json`: `"type": "commonjs"`, `"test": "node --test"`, no dependencies. `node --test` finds `test/*.test.js` by default, so no config is needed.
- `README.md`: "A tiny CommonJS project. No dependencies." The design must keep that.
- No existing source to mirror, so the style is set here: plain functions, `module.exports = { slugify }`, no classes.

## Design

### Algorithm for `slugify(text, options = {})`

1. **Type check:** if `typeof text !== 'string'`, throw `TypeError`.
2. **Options check:** if `options.maxLength` is not `undefined` and is not a positive integer (`Number.isInteger(n) && n > 0`), throw `RangeError`. The task does not define this case. Throwing is safer than silently ignoring a bad limit. This is the one decision the Developer should confirm while implementing.
3. **Accent removal:** `text.normalize('NFD').replace(/[̀-ͯ]/g, '')` (Unicode combining diacritical marks). This turns `é ç ã ü` into their base letters.
4. **Lowercase:** `.toLowerCase()`.
5. **Collapse separators:** `.replace(/[^a-z0-9]+/g, '-')`. Each run of other characters becomes one `-`.
6. **Trim:** `.replace(/^-+|-+$/g, '')`. Given step 5 this is at most one dash per side, but the regex is clearer and safe.
7. **Truncate** (only when `maxLength` is set and `slug.length > maxLength`):
   - `head = slug.slice(0, max)`.
   - If `slug.charAt(max) === '-'`, the cut falls on a word boundary, so return `head`.
   - Else, `idx = head.lastIndexOf('-')`. If `idx > 0`, return `head.slice(0, idx)`. This drops the partial word and any trailing dash.
   - Else (the first word alone is too long), return `head` (hard cut).
   - The result cannot start with `-` (step 6 guarantees the slug does not, and `idx > 0`). It cannot end with `-`: a trailing dash in `head` only happens when `charAt(max) !== '-'`, and then `lastIndexOf` picks it up and slices it off.

Checked by hand against the task examples:

- `'hello big world'`, max 9: slug `hello-big-world`, `charAt(9)` is `-`, result `hello-big`.
- max 8: `head` is `hello-bi`, `charAt(8)` is `g`, `lastIndexOf('-')` is 5, result `hello`.
- `'supercalifragilistic'`, max 5: no `-` in `super`, hard cut, result `super`.
- `'  Olá, Mundo!  '` gives `ola-mundo`. `'Ação & Reação'` gives `acao-reacao`.

### Export

`module.exports = { slugify };` exactly, with no default export.

### Abstractions not added

No helper module, no class, no transliteration map, no separate `truncate` export. Keep everything in one file, with at most two small private helpers if the Developer finds them clearer.

## Files to change

1. `src/slugify.js` (create): the function above, with a short comment on the truncation rule, because the "cut at last fitting `-`" rule is not obvious.
2. `test/slugify.test.js` (create): `const { test } = require('node:test')`, `const assert = require('node:assert')`.

## Test plan (`test/slugify.test.js`)

- Examples from the task: `'  Olá, Mundo!  '`, `'Ação & Reação'`, `'hello big world'` with max 9 and max 8, `'supercalifragilistic'` with max 5.
- Lowercasing: `'HeLLo'` gives `hello`.
- Diacritics: `é ç ã ü` give `e c a u`, for example `'Müller São José'` gives `muller-sao-jose`.
- Separator runs: `'a --- b  c!!d'` gives `a-b-c-d`.
- Leading and trailing junk: `'!!!x???'` gives `x`.
- Empty result: `''` and `'***'` both give `''`.
- No maxLength: a long string is not truncated.
- maxLength equal to the slug length returns it unchanged. maxLength one short of the slug length cuts at the last word (`'hello big world'`, max 14 gives `hello-big`).
- maxLength where the cut lands exactly on `-`: max 9 for `'hello big world'` (covered above).
- Result never ends with `-` for a range of maxLength values: loop max from 1 to the slug length and assert no leading or trailing `-` and length `<= max`.
- Invalid types: `slugify(null)`, `slugify(42)`, `slugify(undefined)` throw `TypeError` (`assert.throws(..., TypeError)`).
- Invalid maxLength: `0`, `-1`, `1.5`, `'5'` throw `RangeError` (only if the Developer keeps the validation in step 2).

## Verification

- `npm test` (runs `node --test`) must pass, with all tests in `test/slugify.test.js` reported.
- Rules check: each of the six numbered rules in the task has at least one test.
- Diff check: `git status` shows only `src/slugify.js` and `test/slugify.test.js` as new files, plus the plan file in `docs/plans/`. Nothing else changes.

## Risks

- **Unicode normalization:** only characters that decompose to ASCII plus a combining mark are handled. This is a documented limitation, not a bug. Non-Latin text will produce an empty or short slug.
- **Empty slug:** an input with no `a-z0-9` yields `''`. Callers must handle it. The task does not say otherwise, so this is kept.
- **Node version:** `String.prototype.normalize` and `node:test` need Node 18+. The project does not pin a version. The `node:test` import in the task already requires 18+.
- **Developer cannot run tests here:** this planning step has no shell. Verification happens in the implementation step.

```atlas-interaction
{"type":"plan","context":"Plan for src/slugify.js and test/slugify.test.js (moderate size); saved to docs/plans/slugify.md."}
```
