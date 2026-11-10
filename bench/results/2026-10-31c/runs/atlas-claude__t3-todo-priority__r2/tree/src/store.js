// Priorities, in sort order (lower rank comes first).
const PRIORITY_RANK = { high: 0, normal: 1, low: 2 };

function checkPriority(priority) {
  if (!Object.prototype.hasOwnProperty.call(PRIORITY_RANK, priority)) {
    throw new RangeError('priority must be one of: high, normal, low');
  }
}

// An in-memory todo store.
function createStore() {
  const todos = [];
  let nextId = 1;

  return {
    /** Adds a todo and returns it. */
    add(title, options) {
      if (typeof title !== 'string' || title.trim() === '') {
        throw new TypeError('title must be a non-empty string');
      }
      const priority = options?.priority === undefined ? 'normal' : options.priority;
      checkPriority(priority);
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

    /**
     * A copy of the todos, high priority first, then normal, then low.
     * Todos with the same priority stay in the order they were added.
     * filter.priority, when given, returns only todos of that priority.
     */
    list(filter) {
      const priority = filter?.priority;
      if (priority !== undefined) checkPriority(priority);
      return todos
        .filter((t) => priority === undefined || t.priority === priority)
        .map((t) => ({ ...t }))
        .sort((a, b) => PRIORITY_RANK[a.priority] - PRIORITY_RANK[b.priority]);
    },
  };
}

module.exports = { createStore };
