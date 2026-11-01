// An in-memory todo store.
function createStore() {
  const todos = [];
  let nextId = 1;
  const priorityOrder = { high: 0, normal: 1, low: 2 };

  return {
    /** Adds a todo and returns it. */
    add(title, options = {}) {
      if (typeof title !== 'string' || title.trim() === '') {
        throw new TypeError('title must be a non-empty string');
      }
      const priority = options.priority === undefined ? 'normal' : options.priority;
      if (!Object.hasOwn(priorityOrder, priority)) {
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

    /** A copy of the todos, filtered or sorted by priority. */
    list(filter) {
      if (filter && filter.priority !== undefined) {
        return todos.filter((t) => t.priority === filter.priority).map((t) => ({ ...t }));
      }

      return todos
        .map((todo, index) => ({ todo, index }))
        .sort((a, b) => priorityOrder[a.todo.priority] - priorityOrder[b.todo.priority] || a.index - b.index)
        .map(({ todo }) => ({ ...todo }));
    },
  };
}

module.exports = { createStore };
