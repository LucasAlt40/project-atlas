// Converts text into a URL-friendly slug: lowercase a-z and 0-9 words joined by '-'.
function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
  }

  const { maxLength } = options;
  if (maxLength !== undefined && (!Number.isInteger(maxLength) || maxLength <= 0)) {
    throw new RangeError('maxLength must be a positive integer');
  }

  // Decompose accents (é -> e + combining mark), drop the marks, then normalise separators.
  const slug = text
    .normalize('NFD')
    .replace(/\p{M}/gu, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  // Cut at the last '-' that fits, so no word is split. If the first word alone is too long, cut it hard.
  const head = slug.slice(0, maxLength + 1);
  const lastDash = head.lastIndexOf('-');
  return lastDash === -1 ? slug.slice(0, maxLength) : head.slice(0, lastDash);
}

module.exports = { slugify };
