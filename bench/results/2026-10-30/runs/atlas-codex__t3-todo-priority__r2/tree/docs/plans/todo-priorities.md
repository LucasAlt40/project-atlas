# Todo Priorities

## Goals

- Extend `createStore().add` with validated `low`, `normal`, and `high` priorities, defaulting to `normal`.
- Preserve each todo's priority in the returned and listed todo objects.
- Support priority filtering and stable priority ordering in `list`.
- Mark high-priority todos with `!` in `formatList` while preserving existing rendering.
- Keep all existing behavior and tests passing, and add focused regression coverage.

## Scope

### In scope

- `src/store.js`: the `add` and `list` contracts, including option validation, filtering, sorting, and copied results.
- `src/format.js`: high-priority display prefix.
- `test/store.test.js`: tests for defaults, validation, stored values, filtering, ordering/stability, and formatting.

### Out of scope

- Persistence, new dependencies, CLI/API changes, or changes to completion/title validation.
- Changing the todo's insertion order in the internal array.
- Formatting changes for `normal` or `low` priorities.

## Design and dependencies

The only runtime boundary is the in-memory store in `src/store.js`; it owns todo creation, IDs, and ordering. `src/format.js` is a pure presentation boundary and should consume the todo shape without importing the store. The test file is the existing contract boundary. There are no production dependencies and tests use Node's built-in runner.

`add(title, options)` should keep the existing title-only call valid by treating a missing options object, and a missing `priority`, as `normal`. Validate any supplied priority against the three allowed strings and throw `RangeError` before mutating the store. Store the normalized priority alongside `id`, `title`, and `done`.

`list(filter)` should continue returning shallow copies. With `filter.priority`, use exact equality as the selection criterion and return only matching todos; no additional filter-validation behavior is required by the task. Without a filter, derive a new result ordered by the explicit rank `high`, `normal`, `low`; use the original array order as the tie-breaker so equal-priority todos remain insertion-stable. Avoid mutating the internal `todos` array. Existing callers that call `list()` with no argument must receive the new priority-ordered behavior required by the task.

`formatList` should retain the existing checkbox and title rendering, adding `!` only when `t.priority === 'high'`, immediately before the title. Existing fixture objects without a priority should continue rendering as before, so the current format test remains valid.

## Steps for the Developer

1. Update `src/store.js` with a local priority validation/ranking mechanism that does not introduce a new module or abstraction.
2. Update `src/format.js` to conditionally prefix only high-priority titles.
3. Extend `test/store.test.js` with focused assertions for the new public behavior and the existing compatibility cases.
4. Run the built-in test command (`npm test`, equivalently `node --test`) and inspect the diff for accidental contract changes.

## Files and contracts to change

- `src/store.js`: `add(title, options)` accepts the optional options object and returns todos with `priority`; `list(filter)` accepts optional `filter.priority`, filters when present, and otherwise returns priority-ranked copies.
- `src/format.js`: `formatList(todos)` emits `[ ] !Title`/`[x] !Title` for high priority and unchanged output for all other or absent priorities.
- `test/store.test.js`: executable contract coverage for default/accepted/rejected priorities, filtering, stable ordering, and high-priority formatting.

## Architectural impact and risks

This is a localized data-shape and presentation change with no new dependency edges. The main risks are accidentally mutating the store while sorting, losing insertion order among equal priorities, rejecting legacy `add(title)` calls, or making the formatter require a priority field that existing callers do not provide. Tests should explicitly guard each of these.

## Verification

- Run `npm test` and require all existing and new tests to pass.
- Confirm tests cover: default `normal`; all three accepted values; `RangeError` for invalid values; priority preserved on add/list; high-normal-low ordering with stable ties; priority filtering; and `!` formatting only for high priority.
- Review the final diff to ensure only the three scoped files and this plan document are touched, with no dependency or API surface additions beyond the requested optional arguments.
