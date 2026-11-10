// Priorities, ranked so that higher-priority todos sort first.
const PRIORITY_RANK = { high: 0, normal: 1, low: 2 };

function checkPriority(priority) {
  if (!Object.hasOwn(PRIORITY_RANK, priority)) {
    throw new RangeError('priority must be one of: low, normal, high');
  }
}

// An in-memory todo store.
function createStore() {
  const todos = [];
  let nextId = 1;

  return {
    /** Adds a todo and returns it. Priority defaults to 'normal'. */
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
     * A copy of the todos, high priority first. Within a priority they keep
     * the order they were added. With filter.priority, only that priority is returned.
     */
    list(filter = {}) {
      if (filter.priority !== undefined) checkPriority(filter.priority);
      return todos
        .filter((t) => filter.priority === undefined || t.priority === filter.priority)
        .sort((a, b) => PRIORITY_RANK[a.priority] - PRIORITY_RANK[b.priority])
        .map((t) => ({ ...t }));
    },
  };
}

module.exports = { createStore };
