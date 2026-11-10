// Priorities from most to least urgent; list() sorts by this order.
const PRIORITIES = ['high', 'normal', 'low'];

// An in-memory todo store.
function createStore() {
  const todos = [];
  let nextId = 1;

  return {
    /** Adds a todo and returns it. options.priority defaults to 'normal'. */
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
     * A copy of the todos, high priority first. Within a priority they keep the
     * order they were added. filter.priority keeps only todos of that priority.
     */
    list(filter = {}) {
      return todos
        .filter((t) => filter.priority === undefined || t.priority === filter.priority)
        .sort((a, b) => PRIORITIES.indexOf(a.priority) - PRIORITIES.indexOf(b.priority))
        .map((t) => ({ ...t }));
    },
  };
}

module.exports = { createStore };
