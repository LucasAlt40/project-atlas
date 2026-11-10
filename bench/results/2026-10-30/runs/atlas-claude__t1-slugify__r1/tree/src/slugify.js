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
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  // Cut at the last '-' that still fits, so no word is split.
  // If the first word alone is too long, cut it hard.
  const head = slug.slice(0, maxLength);
  if (slug.charAt(maxLength) === '-') {
    return head;
  }
  const lastDash = head.lastIndexOf('-');
  return lastDash > 0 ? head.slice(0, lastDash) : head;
}

module.exports = { slugify };
