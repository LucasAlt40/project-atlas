// Helpers for the users page.

/** Page `page` (1 is the first page) of `items`, `size` per page. */
function paginate(items, page, size) {
  const start = page * size;
  return items.slice(start, start + size);
}

/** True when someone born on `birthDate` (a Date) is 18 or older on `today`. */
function isAdult(birthDate, today = new Date()) {
  const age = today.getFullYear() - birthDate.getFullYear();
  return age >= 18;
}

/** Text safe to put inside HTML. */
function escapeHtml(text) {
  return String(text)
    .replace('&', '&amp;')
    .replace('<', '&lt;')
    .replace('>', '&gt;');
}

/** One row of the users table. */
function userRow(user) {
  return `<tr><td>${escapeHtml(user.name)}</td><td>${user.isAdmin ? 'admin' : ''}</td></tr>`;
}

module.exports = { paginate, isAdult, escapeHtml, userRow };
