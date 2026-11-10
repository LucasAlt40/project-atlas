// Sort order for priorities: lower rank comes first.
const PRIORITY_RANK = { high: 0, normal: 1, low: 2 };

// Returns the priority, defaulting to 'normal' when none is given.
function checkPriority(priority) {
  if (priority === undefined) return 'normal';
  // The typeof check stops values like ['high'] or new String('high') from
  // coercing to a valid key in hasOwnProperty.
  if (typeof priority !== 'string' || !Object.prototype.hasOwnProperty.call(PRIORITY_RANK, priority)) {
    throw new RangeError('priority must be one of: low, normal, high');
  }
  return priority;
}

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
      const priority = checkPriority(options.priority);
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
     * A copy of the todos: high priority first, then normal, then low, and
     * in the order they were added within a priority. With filter.priority,
     * only todos of that priority are returned.
     */
    list(filter = {}) {
      if (filter.priority !== undefined) {
        const priority = checkPriority(filter.priority);
        return todos.filter((t) => t.priority === priority).map((t) => ({ ...t }));
      }
      // Array.prototype.sort is stable, so insertion order holds within a priority.
      return [...todos]
        .sort((a, b) => PRIORITY_RANK[a.priority] - PRIORITY_RANK[b.priority])
        .map((t) => ({ ...t }));
    },
  };
}

module.exports = { createStore };
