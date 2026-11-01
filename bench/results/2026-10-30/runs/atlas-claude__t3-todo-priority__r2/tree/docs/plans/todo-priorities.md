# Plan: todo priorities

Size: moderate

## Goals

- Every todo carries a `priority` of `low`, `normal` (default) or `high`.
- `store.list({ priority })` returns only todos of that priority.
- `store.list()` without a filter returns high, then normal, then low; within a priority, insertion order.
- `formatList` marks high-priority todos with `!` before the title (`[ ] !Pay rent`). Other priorities render as today.
- All existing behaviour and tests in `test/` keep working.

## Scope

In scope:

- `src/store.js`: `add(title, options)`, `list(filter)`, a priority constant and rank.
- `src/format.js`: the `!` marker for high priority.
- `test/store.test.js`: new tests for the new behaviour.

Out of scope:

- `complete`, id generation, and the title rules (unchanged).
- Persistence, a CLI, or any other module. None exists in the repo.
- Changing `package.json` or adding dependencies.

## Modules and architectural impact

- `store.js` owns all state (closure over `todos` and `nextId`). Priority is a value of the stored object, so no new module is needed.
- `format.js` has no dependencies today. It should stay that way: it checks `t.priority === 'high'` and does not import from `store.js`. Three priority values do not justify a shared constants module.
- The public `list()` contract changes: its default order moves from insertion order to priority order. This is required by the task. Current callers that only use normal todos see no change, because all of them are the same rank.
- `add` gains an optional second argument. Existing one-argument calls still work.

## Decisions the Developer must follow

1. **Defaults and validation in `add`.** `options` defaults to `{}`. If `options.priority === undefined`, use `'normal'`. Any other value that is not `low`, `normal` or `high` (including `null`, `'HIGH'`, `'urgent'`) throws `RangeError`. Validation happens before the todo is pushed, so a failed call leaves the store unchanged. The existing `TypeError` check on `title` stays as it is and runs first.
2. **Validation in `list` filter.** `filter` defaults to `{}`. `filter.priority === undefined` means no filter. An invalid priority value throws `RangeError`, the same as `add`. The task does not specify this case. Throwing is chosen so that a typo does not silently return an empty list.
3. **Ordering.** Use an explicit rank, `const PRIORITY_RANK = { high: 0, normal: 1, low: 2 }`, and sort with the tie-breaker on `id`: `(a, b) => PRIORITY_RANK[a.priority] - PRIORITY_RANK[b.priority] || a.id - b.id`. This keeps insertion order without relying on sort stability. `id` is monotonic, so it equals insertion order.
4. **One code path.** `list` always does: copy → filter (if a priority is given) → sort. A filtered result is therefore sorted too, which is harmless because all of its items share one rank.
5. **Copies are kept.** `list` still returns `{ ...t }` copies, so callers cannot mutate store state.
6. **Shape.** Every new todo has `{ id, title, done, priority }`. There is no existing persisted data, so no migration is needed.
7. **`formatList` tolerates a missing `priority`.** Todos without the field (such as the ones in the existing `format` test) render as non-high. Output for them is unchanged.
8. **Tests go in `test/store.test.js`.** Use the same flat `test(...)` style and `assert` calls as the file already uses. Do not change the existing four tests.

## Files and contracts to change

- `src/store.js`
  - `add(title, options = {})`: adds `priority` to the stored and returned object; throws `RangeError` on an invalid priority.
  - `list(filter = {})`: returns copies, filtered by `filter.priority` when given, sorted by rank then id.
  - Adds `PRIORITIES` (or `PRIORITY_RANK`) as a module-level constant, not exported unless a test needs it. Tests should use string literals.
- `src/format.js`
  - `formatList`: prefix `!` to the title when `t.priority === 'high'`. The bracket part is unchanged: `[x] !title` or `[ ] !title`.
- `test/store.test.js`
  - New tests (listed below). Existing tests unchanged.
- `README.md`: optional one-line note on the new option. Only if the Developer thinks it helps; not required.

Public contract after the change:

- `add(title: string, options?: { priority?: 'low'|'normal'|'high' }) -> { id, title, done, priority }`
- `list(filter?: { priority?: 'low'|'normal'|'high' }) -> Array<{ id, title, done, priority }>`
- `formatList(todos) -> string`, with `!` for `priority === 'high'`.

## Tests to add (test/store.test.js)

- `add` defaults to `normal` when no options are given, and when `options` is `{}`.
- `add` stores `low`, `normal` and `high` in `priority`.
- `add` with an invalid priority (`'urgent'`, `'HIGH'`, `null`) throws `RangeError`, and `list()` is unchanged afterwards.
- `list()` sorts high, normal, low; within each priority, insertion order is kept (for example: add `n1`, `h1`, `l1`, `h2`, `n2` → `h1, h2, n1, n2, l1`).
- `list({ priority: 'low' })` returns only low todos, in insertion order.
- `list({ priority: 'bogus' })` throws `RangeError`.
- `list()` and `list({})` return the same result.
- `formatList`: a high todo renders as `[ ] !title`; a done high todo renders as `[x] !title`; normal and low todos, and todos with no `priority` field, render exactly as before.

## Verification

- Run `npm test` (`node --test`). All existing four tests and the new ones must pass.
- Confirm by reading that the existing `add and list` test still passes: all todos there are normal, so the sort keeps insertion order.
- Note: in this step I cannot run commands, so no tests have been run. The Developer step must run `npm test` and report the output.

## Risks

- **Changed `list()` order.** Anything that relied on insertion order for mixed priorities will see a different order. This is the requested behaviour, but the user should know it.
- **Duplicated literal.** The string `'high'` appears in `format.js` and `store.js`. If a priority is ever renamed, both files must change. The cost is small; a shared module would add more than it saves now.
- **Filter validation is a choice.** The task does not say what `list({ priority: 'bogus' })` should do. The plan throws. If the user wants an empty list instead, this is a one-line change.
- **Mutable options object.** `add` reads `options.priority` once, so later changes to the caller's object do not affect the stored todo.
