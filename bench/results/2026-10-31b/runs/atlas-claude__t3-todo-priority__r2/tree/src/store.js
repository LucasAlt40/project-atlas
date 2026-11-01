// Allowed priorities, lowest first. A todo's rank is its index here.
const PRIORITIES = ['low', 'normal', 'high'];

function checkPriority(priority) {
  if (!PRIORITIES.includes(priority)) {
    throw new RangeError(`priority must be one of: ${PRIORITIES.join(', ')}`);
  }
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
      // Only undefined means "not given"; null is passed on and rejected by checkPriority.
      const { priority = 'normal' } = options ?? {};
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

    /**
     * A copy of the todos. With filter.priority, only todos of that priority.
     * Otherwise high first, then normal, then low; ties keep the order they were added.
     */
    list(filter) {
      const { priority } = filter ?? {};
      if (priority !== undefined) {
        checkPriority(priority);
      }
      const rank = (t) => PRIORITIES.indexOf(t.priority);
      return todos
        .filter((t) => priority === undefined || t.priority === priority)
        .sort((a, b) => rank(b) - rank(a))
        .map((t) => ({ ...t }));
    },
  };
}

module.exports = { createStore };
