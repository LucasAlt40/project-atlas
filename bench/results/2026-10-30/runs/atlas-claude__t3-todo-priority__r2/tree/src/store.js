// An in-memory todo store.
const PRIORITY_RANK = { high: 0, normal: 1, low: 2 };

function checkPriority(priority) {
  if (!Object.prototype.hasOwnProperty.call(PRIORITY_RANK, priority)) {
    throw new RangeError('priority must be low, normal or high');
  }
}

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

    /** A copy of the todos, high priority first, then in the order they were added. */
    list(filter = {}) {
      let result = todos.map((t) => ({ ...t }));
      if (filter.priority !== undefined) {
        checkPriority(filter.priority);
        result = result.filter((t) => t.priority === filter.priority);
      }
      return result.sort(
        (a, b) => PRIORITY_RANK[a.priority] - PRIORITY_RANK[b.priority] || a.id - b.id,
      );
    },
  };
}

module.exports = { createStore };
