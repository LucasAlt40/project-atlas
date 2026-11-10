// Priorities, highest first. A todo's rank is its index here.
const PRIORITIES = ['high', 'normal', 'low'];

function assertPriority(priority) {
  if (!PRIORITIES.includes(priority)) {
    throw new RangeError(`priority must be one of ${PRIORITIES.join(', ')}`);
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
      assertPriority(priority);
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
     * Otherwise sorted high first, then normal, then low; ties keep the order they were added.
     */
    list(filter = {}) {
      if (filter.priority !== undefined) assertPriority(filter.priority);
      const selected = filter.priority === undefined
        ? todos.slice().sort((a, b) => PRIORITIES.indexOf(a.priority) - PRIORITIES.indexOf(b.priority))
        : todos.filter((t) => t.priority === filter.priority);
      return selected.map((t) => ({ ...t }));
    },
  };
}

module.exports = { createStore };
