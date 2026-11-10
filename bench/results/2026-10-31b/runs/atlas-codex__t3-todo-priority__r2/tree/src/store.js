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
      const priority = options?.priority === undefined ? 'normal' : options.priority;
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

    /** A copy of the todos, ordered by priority unless filtered. */
    list(filter) {
      const result = todos
        .filter((todo) => filter?.priority === undefined || todo.priority === filter.priority)
        .map((todo) => ({ ...todo }));

      if (filter?.priority !== undefined) return result;

      const priorityOrder = { high: 0, normal: 1, low: 2 };
      return result
        .map((todo, index) => ({ todo, index }))
        .sort((a, b) => priorityOrder[a.todo.priority] - priorityOrder[b.todo.priority] || a.index - b.index)
        .map(({ todo }) => todo);
    },
  };
}

module.exports = { createStore };
