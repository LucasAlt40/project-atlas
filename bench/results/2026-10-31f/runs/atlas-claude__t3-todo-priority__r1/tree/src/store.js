// An in-memory todo store.
const PRIORITIES = ['low', 'normal', 'high'];

// Higher priority sorts first.
const RANK = { high: 0, normal: 1, low: 2 };

function checkPriority(priority) {
  if (!PRIORITIES.includes(priority)) {
    throw new RangeError(`priority must be one of: ${PRIORITIES.join(', ')}`);
  }
}

function createStore() {
  const todos = [];
  let nextId = 1;

  return {
    /** Adds a todo and returns it. */
    add(title, options = {}) {
      if (typeof title !== 'string' || title.trim() === '') {
        throw new TypeError('title must be a non-empty string');
      }
      const priority = options.priority === undefined ? 'normal' : options.priority;
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
     * Todos of the same priority keep the order they were added in.
     * filter.priority, when given, returns only todos of that priority.
     */
    list(filter = {}) {
      if (filter.priority !== undefined) checkPriority(filter.priority);
      return todos
        .filter((t) => filter.priority === undefined || t.priority === filter.priority)
        .map((t) => ({ ...t }))
        .sort((a, b) => RANK[a.priority] - RANK[b.priority]);
    },
  };
}

module.exports = { createStore };
