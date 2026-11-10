// An in-memory todo store.
function createStore() {
  const todos = [];
  let nextId = 1;
  const priorityRank = { high: 0, normal: 1, low: 2 };

  return {
    /** Adds a todo and returns it. */
    add(title, options) {
      if (typeof title !== 'string' || title.trim() === '') {
        throw new TypeError('title must be a non-empty string');
      }
      const priority = options?.priority === undefined ? 'normal' : options.priority;
      if (!Object.hasOwn(priorityRank, priority)) {
        throw new RangeError('priority must be low, normal, or high');
      }
      const todo = { id: nextId++, title: title.trim(), done: false, priority };
      todos.push(todo);
      return todo;
    },

    /** Marks a todo as done. Returns false when there is no such todo. */
    complete(id) {
      const todo = todos.find((t) => t.id === id);
      if (!todo) return false;
      todo.done = true;
      return true;
    },

    /** A copy of the todos, ordered by priority and optionally filtered. */
    list(filter) {
      const filtered =
        filter?.priority === undefined
          ? todos
          : todos.filter((todo) => todo.priority === filter.priority);

      return filtered
        .map((todo, index) => ({ todo, index }))
        .sort(
          (a, b) =>
            priorityRank[a.todo.priority] - priorityRank[b.todo.priority] || a.index - b.index,
        )
        .map(({ todo }) => ({ ...todo }));
    },
  };
}

module.exports = { createStore };
