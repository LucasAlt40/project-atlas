function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('slugify: text must be a string');
  }

  const { maxLength } = options;
  if (maxLength !== undefined && !(Number.isInteger(maxLength) && maxLength > 0)) {
    throw new RangeError('slugify: maxLength must be a positive integer');
  }

  const slug = text
    .normalize('NFD')
    .replace(/\p{M}/gu, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  // Cut at a word boundary when the cut point falls on a '-'.
  if (slug[maxLength] === '-') {
    return slug.slice(0, maxLength);
  }

  const head = slug.slice(0, maxLength);
  const lastDash = head.lastIndexOf('-');
  // Keep the first word whole when it fits; otherwise cut it hard.
  return lastDash > 0 ? head.slice(0, lastDash) : head;
}

module.exports = { slugify };
