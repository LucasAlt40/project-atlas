// An in-memory todo store.
function createStore() {
  const todos = [];
  let nextId = 1;
  const priorities = { high: 0, normal: 1, low: 2 };

  return {
    /** Adds a todo and returns it. */
    add(title, options = {}) {
      if (typeof title !== 'string' || title.trim() === '') {
        throw new TypeError('title must be a non-empty string');
      }
      const priority = options.priority === undefined ? 'normal' : options.priority;
      if (!Object.hasOwn(priorities, priority)) {
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

    /** A copy of the todos, sorted by priority unless filtered. */
    list(filter) {
      const selected = filter?.priority
        ? todos.filter((t) => t.priority === filter.priority)
        : [...todos].sort((a, b) => priorities[a.priority] - priorities[b.priority]);
      return selected.map((t) => ({ ...t }));
    },
  };
}

module.exports = { createStore };
