// An in-memory todo store.
function createStore() {
  const todos = [];
  let nextId = 1;
  const priorityRank = { high: 0, normal: 1, low: 2 };

  return {
    /** Adds a todo and returns it. */
    add(title, options = {}) {
      if (typeof title !== 'string' || title.trim() === '') {
        throw new TypeError('title must be a non-empty string');
      }
      const priority = options.priority === undefined ? 'normal' : options.priority;
      if (!['low', 'normal', 'high'].includes(priority)) {
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

    /** A copy of the todos, filtered and ordered by priority. */
    list(filter) {
      let result = todos;
      if (filter && filter.priority !== undefined) {
        result = todos.filter((t) => t.priority === filter.priority);
      } else {
        result = [...todos].sort((a, b) => priorityRank[a.priority] - priorityRank[b.priority]);
      }
      return result.map((t) => ({ ...t }));
    },
  };
}

module.exports = { createStore };
