const PRIORITIES = ['high', 'normal', 'low'];

function checkPriority(priority) {
  if (!PRIORITIES.includes(priority)) {
    throw new RangeError(`priority must be one of: ${PRIORITIES.join(', ')}`);
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
     * A copy of the todos. With filter.priority, only todos of that priority.
     * Otherwise sorted high, normal, low; ties keep the order they were added.
     */
    list(filter = {}) {
      if (filter.priority !== undefined) {
        checkPriority(filter.priority);
        return todos.filter((t) => t.priority === filter.priority).map((t) => ({ ...t }));
      }
      const rank = (t) => PRIORITIES.indexOf(t.priority);
      return todos
        .map((t) => ({ ...t }))
        .sort((a, b) => rank(a) - rank(b));
    },
  };
}

module.exports = { createStore };
