# Slugify Module Plan

## Goals

- Add a dependency-free CommonJS `slugify(text, options)` utility.
- Normalize input to lowercase ASCII slugs by removing diacritics and collapsing non-alphanumeric runs.
- Enforce the specified `maxLength` truncation behavior without splitting words unless the first word itself is too long.
- Cover the public contract with built-in `node:test` and `node:assert` tests.

## Scope

In scope:

- `src/slugify.js`, exporting exactly `{ slugify }`.
- `test/slugify.test.js`, covering normalization, separators, trimming, truncation, and invalid input.

Out of scope:

- Changes to `package.json`, `README.md`, or any other existing project file.
- Third-party dependencies, configuration changes, browser/UI integration, or unrelated text utilities.

## Steps

1. Implement input validation and Unicode normalization using platform APIs already available in Node.js.
2. Convert normalized text to lowercase ASCII, replace each run outside `[a-z0-9]` with one separator, and trim boundary separators.
3. Apply `options.maxLength` only when supplied as a positive integer, selecting the last separator that fits and hard-cutting an overlong first word.
4. Add focused tests using `node:test` and `node:assert`, including the supplied examples and boundary cases.
5. Run the repository test command and inspect the final diff to verify only the requested implementation and test files are changed.

## Files and Contracts

- `src/slugify.js`: CommonJS module whose public export is exactly `module.exports = { slugify }`; non-string `text` throws `TypeError`.
- `test/slugify.test.js`: imports the named `slugify` export and asserts the observable rules without coupling to internal helpers.
- `package.json`: existing `test` script (`node --test`) is reused unchanged.

## Architectural Impact

This is an isolated leaf utility in the existing dependency-free text-utils package. It introduces no new runtime dependency or cross-module coupling. The main design boundary is between normalization/slug construction and optional length policy; keeping those concerns local to the function avoids abstractions that the project does not need.

## Decisions the Developer Must Follow

- Use only built-in JavaScript/Node functionality; do not add packages or alter project configuration.
- Treat `text` validation as a public API precondition and throw `TypeError` before processing invalid values.
- Use Unicode decomposition plus removal of combining marks for accents and diacritics, then lowercase before ASCII filtering.
- Collapse every non-`a-z`/`0-9` run, including punctuation, whitespace, symbols, and unsupported characters, into one `-`; trim any resulting boundary separator.
- For a valid positive-integer `maxLength`, preserve whole hyphen-delimited words where a separator fits; hard-cut only when the first word alone exceeds the limit. Do not let the returned value end in `-`.
- Do not invent behavior for malformed option shapes beyond the stated optional positive-integer contract unless tests or existing project conventions require it.

## Risks and Trade-offs

- Unicode normalization handles decomposable accents reliably, while characters without a simple decomposition remain filtered as non-ASCII; this follows the explicit ASCII output contract but may remove rather than transliterate some scripts.
- A truncation implementation that slices first and trims later can accidentally split a word or leave a trailing separator, so tests must exercise exact separator and first-word boundaries.
- The repository has no existing source/test patterns to reuse beyond its CommonJS and built-in test configuration; the implementation should therefore remain small and explicit.

## Verification

- Run `npm test` (equivalent to `node --test`).
- Confirm the supplied examples pass, plus tests for non-string input, empty/separator-only input, accents, repeated separators, exact length, a boundary that would end on `-`, and an overlong first word.
- Review `git diff --stat` and `git status --short` to ensure production/test changes are limited to the requested paths; the plan itself is a workflow artifact.
