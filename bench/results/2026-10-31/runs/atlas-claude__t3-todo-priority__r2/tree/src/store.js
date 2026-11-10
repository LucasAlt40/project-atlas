// An in-memory todo store.
const PRIORITIES = ['high', 'normal', 'low'];

function createStore() {
  const todos = [];
  let nextId = 1;

  return {
    /** Adds a todo and returns it. options.priority is 'low', 'normal' (default) or 'high'. */
    add(title, options = {}) {
      if (typeof title !== 'string' || title.trim() === '') {
        throw new TypeError('title must be a non-empty string');
      }
      const priority = options.priority === undefined ? 'normal' : options.priority;
      if (!PRIORITIES.includes(priority)) {
        throw new RangeError(`priority must be one of: ${PRIORITIES.join(', ')}`);
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
     * Otherwise sorted high, normal, low; within a priority, in the order they were added.
     */
    list(filter = {}) {
      if (filter.priority !== undefined) {
        return todos.filter((t) => t.priority === filter.priority).map((t) => ({ ...t }));
      }
      return [...todos]
        .sort((a, b) => PRIORITIES.indexOf(a.priority) - PRIORITIES.indexOf(b.priority))
        .map((t) => ({ ...t }));
    },
  };
}

module.exports = { createStore };
