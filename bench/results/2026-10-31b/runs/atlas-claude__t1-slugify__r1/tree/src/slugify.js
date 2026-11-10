function slugify(text, options) {
  if (typeof text !== 'string') {
    throw new TypeError('slugify: text must be a string');
  }

  const { maxLength } = options || {};
  if (maxLength !== undefined && (!Number.isInteger(maxLength) || maxLength <= 0)) {
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

  // Cut at the last '-' that fits; if the first word alone is too long, cut it hard.
  const cut = slug.lastIndexOf('-', maxLength);
  return cut === -1 ? slug.slice(0, maxLength) : slug.slice(0, cut);
}

module.exports = { slugify };
