// Plain-text rendering of todos, one per line.
function formatList(todos) {
  return todos.map((t) => `[${t.done ? 'x' : ' '}] ${t.priority === 'high' ? '! ' : ''}${t.title}`).join('\n');
}

module.exports = { formatList };
