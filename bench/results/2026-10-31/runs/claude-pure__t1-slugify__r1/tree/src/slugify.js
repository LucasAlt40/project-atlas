function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
  }

  const slug = text
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  const { maxLength } = options;
  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  if (slug[maxLength] === '-') {
    return slug.slice(0, maxLength);
  }

  const head = slug.slice(0, maxLength);
  const lastDash = head.lastIndexOf('-');
  return lastDash > 0 ? head.slice(0, lastDash) : head;
}

module.exports = { slugify };
