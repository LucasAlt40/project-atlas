const PRIORITIES = ['low', 'normal', 'high'];

// Sort order for list(): high first, then normal, then low.
const RANK = { high: 0, normal: 1, low: 2 };

// An in-memory todo store.
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
      if (!PRIORITIES.includes(priority)) {
        throw new RangeError(`priority must be one of ${PRIORITIES.join(', ')}`);
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

    /**
     * A copy of the todos. With filter.priority, only todos of that priority.
     * Otherwise sorted by priority (high first); ties keep the order they were added.
     */
    list(filter = {}) {
      const matching = filter.priority === undefined
        ? todos
        : todos.filter((t) => t.priority === filter.priority);
      // Array.prototype.sort is stable, so equal priorities keep insertion order.
      return matching
        .slice()
        .sort((a, b) => RANK[a.priority] - RANK[b.priority])
        .map((t) => ({ ...t }));
    },
  };
}

module.exports = { createStore };
