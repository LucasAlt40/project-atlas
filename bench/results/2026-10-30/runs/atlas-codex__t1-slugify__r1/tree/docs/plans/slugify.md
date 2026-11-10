# Slugify Module Plan

## Goals

- Add a dependency-free CommonJS `slugify(text, options)` utility.
- Normalize text to lowercase ASCII slugs by removing diacritics, collapsing non-alphanumeric runs to `-`, and trimming separators.
- Enforce `options.maxLength` without splitting words when a separator boundary fits, while hard-cutting an overlong first word as specified.
- Add focused built-in `node:test` coverage for the public contract and edge cases.

## Steps

1. Create `src/slugify.js` with a named `slugify` function and the exact export `module.exports = { slugify }`.
2. Validate the input type before normalization; normalize accents/diacritics through Unicode decomposition and removal of combining marks, then apply the separator and trim rules.
3. Apply `maxLength` only when it is present as a positive integer, selecting the longest complete word sequence that fits and hard-cutting only when the first word itself exceeds the limit.
4. Create `test/slugify.test.js` using `node:test` and `node:assert`, covering examples, normalization, separators, trimming, type errors, and length-boundary behavior.
5. Run the package test script (`node --test` via `npm test`) and inspect the final diff to confirm only the requested implementation/test files are part of the feature change.

## Files to change

- `src/slugify.js`: new public utility module.
- `test/slugify.test.js`: new contract tests.

This plan is documented under `docs/plans/` because the workflow requires analysis plans to be saved. The implementation step must not modify unrelated project files.

## Risks

- Unicode decomposition does not transliterate every non-Latin script; the specified contract only requires removal of accents/diacritics, after which unsupported characters become separators.
- A malformed `maxLength` option needs a deliberate compatibility decision; the stated contract defines behavior only for an optional positive integer, so tests should avoid inventing coercion semantics unless the implementation documents a clear choice.
- Hard-cutting the first word must not leave a trailing separator; the length logic should operate on normalized words rather than a post-trimmed arbitrary string.

## Verification

- Run `npm test` / `node --test`.
- Assert the supplied examples exactly.
- Assert punctuation/whitespace collapsing, Unicode accent removal, empty/separator-only input, non-string `TypeError`, and both word-boundary and hard-cut `maxLength` cases.
- Review `git diff --stat` and `git status --short` to ensure the feature implementation is limited to `src/slugify.js` and `test/slugify.test.js` in addition to this required plan artifact.
