// An in-memory todo store.
const PRIORITIES = ['high', 'normal', 'low'];

function createStore() {
  const todos = [];
  let nextId = 1;

  return {
    /** Adds a todo and returns it. priority is 'low', 'normal' (the default) or 'high'. */
    add(title, { priority = 'normal' } = {}) {
      if (typeof title !== 'string' || title.trim() === '') {
        throw new TypeError('title must be a non-empty string');
      }
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
     * A copy of the todos, optionally only those with filter.priority. Sorted high first,
     * then normal, then low; within a priority, in the order they were added.
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
