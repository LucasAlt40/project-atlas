# Plan: todo priorities

Size: moderate

## Goals

- Each todo carries a `priority` of `'low'`, `'normal'` or `'high'` (default `'normal'`).
- `store.list({ priority })` returns only todos of that priority.
- `store.list()` without a filter returns todos sorted high, normal, low; ties keep insertion order.
- `formatList` prefixes high-priority titles with `!` (`[ ] !Pay rent`). Other priorities render as before.
- Existing behaviour and the existing tests in `test/store.test.js` stay unchanged.

## Out of scope

- Changing `complete()`, the `done` flag, or the id scheme.
- Persistence, CLI, or README changes (the README is a one-line description and needs no update).
- Sorting or filtering by anything other than priority.
- Changing `list()`'s return type: it still returns copies, not internal objects.

## Modules and dependencies

- `src/store.js`: the only owner of todo state (closure over `todos` and `nextId`). Priority validation, defaulting, sorting and filtering belong here.
- `src/format.js`: pure rendering. It has no dependency on `store.js` and must stay that way. It only needs to know the literal `'high'`.
- `test/store.test.js`: the single test file, covering both modules. No new test dependencies. `package.json` already runs `node --test`.

Architectural impact: none beyond `store.js`'s public surface. `list` gains an optional argument, which is backward compatible. The `formatList` input shape is unchanged; the `priority` field is optional on input.

## Decisions for the Developer

1. **Priority order is one constant in `store.js`**, e.g. `const PRIORITIES = ['high', 'normal', 'low'];`. Validation uses `includes`, and sort rank uses `indexOf`. Do not export it and do not add a separate module: nothing outside `store.js` needs it.
2. **Defaulting uses an explicit `undefined` check**, not `??` or `||`. `add('a', { priority: undefined })` and `add('a')` give `'normal'`. `null`, `''`, `'urgent'` and any other value throw `RangeError`. This matches "any other value throws" in the spec.
3. **`add(title, options = {})`**. The existing `TypeError` for the title is kept and checked first.
4. **`list(filter = {})`**. If `filter.priority !== undefined`, it must be a valid priority (otherwise `RangeError`, the same rule as `add`). This is my choice; the spec does not cover an invalid filter value, and throwing is more consistent than silently returning `[]`.
5. **Sorting happens on the copied array** returned by `list()`, never on the internal `todos` array. Use `Array.prototype.sort` with the rank comparator; it is stable in all supported Node versions, so ties keep insertion order. Do not add an `index` field.
6. **`formatList` tests `t.priority === 'high'`**, not `!t.priority`. The existing format test passes todos with no `priority` field, and they must still print `[x] a` / `[ ] b`.
7. **Every todo object gets `priority`**, set in `add`, so the returned object and `list()` copies include it.

## Files and contracts to change

- `src/store.js`
  - `add(title, options = {})` → returns `{ id, title, done, priority }`.
  - `list(filter = {})` → returns sorted copies, filtered by `filter.priority` when given.
- `src/format.js`
  - `formatList(todos)`: each line is `[x] !title` for high, `[ ] title` otherwise.
- `test/store.test.js`
  - Keep the four existing tests unchanged.
  - Add tests (see below).

## New tests (in `test/store.test.js`)

- `add` defaults priority to `'normal'`.
- `add(title, { priority })` stores `'low'`, `'normal'`, `'high'`.
- `add` with `'urgent'`, `null` and `''` throws `RangeError`.
- `list()` sorts high, normal, low; within a group, insertion order is kept.
- `list({ priority: 'low' })` returns only low todos, in insertion order.
- `list({ priority: 'urgent' })` throws `RangeError`.
- `list()` does not reorder the internal store: a second call returns the same order as the first.
- `formatList` prints `[ ] !Pay rent` for high and plain `[ ] title` for the other priorities, and the existing format test still passes.

## Risks

- **Silent behaviour change in `list()`**: the default order now differs from insertion order whenever priorities are mixed. This is what the spec asks for; the existing "add and list" test only uses default priority, so it still passes.
- **Null vs undefined in `add`**: `null` throws under decision 2. A caller passing `null` to mean "default" gets an error. This is intended by the spec, but it should be stated in the final summary.
- **`formatList` on old-shape objects**: handled by decision 6.

## Verification

- `npm test` (runs `node --test`) must pass: the 4 existing tests plus the new ones.
- Each new test maps to one bullet in the spec.
- Reviewer check: `src/store.js` has no sort on the internal `todos` array, and `src/format.js` does not require `store.js`.

I could not run `npm test` in this step (no shell access). The plan is verified by reading the code only.
