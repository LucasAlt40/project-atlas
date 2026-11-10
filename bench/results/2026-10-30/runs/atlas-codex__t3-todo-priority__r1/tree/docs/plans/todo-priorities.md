# Plan: Add priorities to todos

## Goals

- Extend `store.add(title, options)` with a validated `priority` option whose values are `low`, `normal`, or `high`, defaulting to `normal`.
- Preserve each todo's priority in its public `priority` field.
- Extend `store.list(filter)` with priority filtering and priority-ranked output while preserving insertion order within a priority.
- Render high-priority todos with `!` immediately before the title while leaving other output unchanged.
- Keep all existing behavior and tests passing, and add focused tests for the new contracts.

## Scope

In scope:

- `src/store.js`: add-option validation, todo shape, list filtering, and stable priority ordering.
- `src/format.js`: high-priority marker in `formatList`.
- `test/store.test.js` or an additional test file under `test/`: regression and feature coverage for defaults, validation, filtering, ordering, and formatting.

Out of scope:

- Persistence, a new module, changes to IDs/completion/title validation, CLI/UI work, or dependency changes.
- Backfilling or migrating external data; this store is in-memory and currently creates all todo objects itself.

## Modules, dependencies, and architectural impact

`src/store.js` remains the domain boundary for todo creation and retrieval. It owns the allowed priority vocabulary, defaulting, validation, and ordering because those rules define the store's API semantics. The existing array remains the source of insertion order; list results can be derived from it without changing mutation behavior. No new dependency or abstraction is needed.

`src/format.js` remains a pure presentation boundary. It should inspect `todo.priority` only to add the marker and otherwise retain the current checkbox/title format. The formatter must tolerate existing todo-shaped inputs without a priority field so the existing format test and callers are not needlessly broken.

`test/store.test.js` is the current contract test boundary and already imports both modules, so focused additions can reuse the existing patterns and built-in `node:test` runner.

## Decisions the Developer must follow

1. Change the store signature to `add(title, options)` while treating omitted options as `{}`; default only a missing/undefined priority to `normal`.
2. Accept exactly the strings `low`, `normal`, and `high`; reject every other value with `RangeError`. Preserve the existing `TypeError` for invalid titles. Do not silently coerce values.
3. Store `priority` on every newly created todo, including the defaulted value.
4. Make `list(filter)` optional. With `filter.priority`, return only todos whose priority strictly matches the requested value; do not add an unspecified exception for invalid filter values. With no filter, order by `high`, `normal`, then `low`, preserving original insertion order among equal priorities. Returned objects must remain shallow copies, as today.
5. Keep list ordering deterministic without mutating the internal `todos` array; completion and future operations should continue to address the same todo objects by ID.
6. In `formatList`, prefix only high-priority titles with `!` (`[ ] !Pay rent`); normal, low, and legacy objects without a priority retain the old rendering.
7. Tests should explicitly cover default priority, all valid values, invalid add priorities, high/normal/low ordering with ties, priority filtering, and high-only formatting, while retaining the current tests.

## Files and contracts to change

- `src/store.js`
  - `add(title, options)`: accepts options and returns `{ id, title, done, priority }`.
  - `list(filter)`: returns copied todos, filtered when requested, ranked high-to-low with stable same-priority order.
- `src/format.js`
  - `formatList(todos)`: keeps existing line format and inserts `!` only for `priority === 'high'`.
- `test/store.test.js` (or focused new tests under `test/`)
  - Encodes the new API and guards existing behavior.
- `docs/plans/todo-priorities.md`
  - Records this approved implementation plan for workflow handoff.

## Risks and trade-offs

- Sorting a copied list is simple and preserves encapsulation, but it adds `O(n log n)` work per unfiltered `list` call; this is appropriate for the existing in-memory, tiny store and avoids maintaining a second index.
- JavaScript's stable sort behavior is relied on only if the implementation uses a comparator that returns equality for matching priorities; alternatively, an explicit rank-plus-original-index approach can make the insertion-order guarantee obvious. Do not mutate `todos` while sorting.
- Invalid `filter.priority` behavior is not specified; strict matching naturally produces no matches without expanding the public API with a new exception contract.

## Verification

- Run `npm test` with the existing Node test runner.
- Confirm existing title validation, completion, insertion-related behavior, and legacy formatting still pass.
- Confirm feature tests prove defaulting, rejection with `RangeError`, stable priority ordering, filtering, and the exact high-priority marker.
- Review the diff to ensure only the listed source/test/plan files are touched and no dependency or server changes are introduced.
