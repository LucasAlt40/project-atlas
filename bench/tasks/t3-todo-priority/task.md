Add priorities to the todo store in this project (src/store.js and src/format.js).

- store.add(title, options): options.priority is 'low', 'normal' or 'high'. It defaults to 'normal'. Any other value throws a RangeError. Each todo keeps its priority in a `priority` field.
- store.list(filter): with filter.priority, only todos of that priority are returned. Without a filter, todos are sorted high first, then normal, then low; within the same priority, they keep the order in which they were added.
- formatList(todos) in src/format.js shows high-priority todos with a '!' right before the title, e.g. '[ ] !Pay rent'. Other priorities are shown as before.

Everything that works today must keep working, including the existing tests in test/. Add tests for the new behaviour in test/.
