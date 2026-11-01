// An in-memory todo store.
function createStore() {
  const todos = [];
  let nextId = 1;
  const priorities = ['high', 'normal', 'low'];

  return {
    /** Adds a todo and returns it. */
    add(title, options = {}) {
      if (typeof title !== 'string' || title.trim() === '') {
        throw new TypeError('title must be a non-empty string');
      }
      const priority = options.priority === undefined ? 'normal' : options.priority;
      if (!priorities.includes(priority)) {
        throw new RangeError('priority must be low, normal, or high');
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

    /** A copy of the todos, filtered or ordered by priority. */
    list(filter) {
      const copies = todos.map((t) => ({ ...t }));
      if (filter && filter.priority !== undefined) {
        return copies.filter((t) => t.priority === filter.priority);
      }
      return copies.sort((a, b) => priorities.indexOf(a.priority) - priorities.indexOf(b.priority));
    },
  };
}

module.exports = { createStore };
