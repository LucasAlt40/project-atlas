// Sort order for priorities: high first.
const PRIORITY_RANK = { high: 0, normal: 1, low: 2 };

// An in-memory todo store.
function createStore() {
  const todos = [];
  let nextId = 1;

  function checkPriority(priority) {
    if (!Object.prototype.hasOwnProperty.call(PRIORITY_RANK, priority)) {
      throw new RangeError('priority must be one of low, normal, high');
    }
  }

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
     * A copy of the todos, high priority first, then in the order they were added.
     * With filter.priority, only todos of that priority are returned.
     */
    list(filter = {}) {
      if (filter.priority !== undefined) checkPriority(filter.priority);
      return todos
        .filter((t) => filter.priority === undefined || t.priority === filter.priority)
        .map((t) => ({ ...t }))
        .sort((a, b) => PRIORITY_RANK[a.priority] - PRIORITY_RANK[b.priority]);
    },
  };
}

module.exports = { createStore };
