const PRIORITIES = ['low', 'normal', 'high'];
const RANK = { high: 0, normal: 1, low: 2 };

// An in-memory todo store.
function createStore() {
  const todos = [];
  let nextId = 1;

  return {
    /** Adds a todo and returns it. options.priority defaults to 'normal'. */
    add(title, options) {
      if (typeof title !== 'string' || title.trim() === '') {
        throw new TypeError('title must be a non-empty string');
      }
      const priority = options === undefined || options.priority === undefined ? 'normal' : options.priority;
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
     * A copy of the todos, high priority first, then normal, then low.
     * Within a priority they keep the order they were added. filter.priority
     * limits the result to one priority.
     */
    list(filter = {}) {
      const priority = filter.priority;
      if (priority !== undefined && !PRIORITIES.includes(priority)) {
        throw new RangeError(`priority must be one of ${PRIORITIES.join(', ')}`);
      }
      return todos
        .filter((t) => priority === undefined || t.priority === priority)
        .sort((a, b) => RANK[a.priority] - RANK[b.priority])
        .map((t) => ({ ...t }));
    },
  };
}

module.exports = { createStore };
