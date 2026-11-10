// Priorities, highest first. A priority's index is its sort rank.
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
    /** Adds a todo and returns it. */
    add(title, options = {}) {
      if (typeof title !== 'string' || title.trim() === '') {
        throw new TypeError('title must be a non-empty string');
      }
      const { priority = 'normal' } = options;
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
     * A copy of the todos. With filter.priority, only that priority is returned.
     * Otherwise they are sorted high first. Within a priority, insertion order is kept.
     */
    list(filter = {}) {
      if (filter.priority !== undefined) {
        checkPriority(filter.priority);
        return todos.filter((t) => t.priority === filter.priority).map((t) => ({ ...t }));
      }
      return todos
        .map((t) => ({ ...t }))
        .sort((a, b) => PRIORITIES.indexOf(a.priority) - PRIORITIES.indexOf(b.priority));
    },
  };
}

module.exports = { createStore };
