// Lower rank sorts first.
const PRIORITY_RANK = { high: 0, normal: 1, low: 2 };

function isPriority(value) {
  return Object.prototype.hasOwnProperty.call(PRIORITY_RANK, value);
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
      if (!isPriority(priority)) {
        throw new RangeError(`priority must be 'low', 'normal' or 'high'`);
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
     * A copy of the todos, sorted high → normal → low and, within a priority,
     * in the order they were added. filter.priority limits the result to one priority.
     */
    list(filter = {}) {
      if (filter.priority !== undefined && !isPriority(filter.priority)) {
        throw new RangeError(`priority must be 'low', 'normal' or 'high'`);
      }
      return todos
        .filter((t) => filter.priority === undefined || t.priority === filter.priority)
        .sort((a, b) => PRIORITY_RANK[a.priority] - PRIORITY_RANK[b.priority])
        .map((t) => ({ ...t }));
    },
  };
}

module.exports = { createStore };
