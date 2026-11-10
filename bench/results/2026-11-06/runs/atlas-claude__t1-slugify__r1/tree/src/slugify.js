function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
  }

  const { maxLength } = options;
  if (maxLength !== undefined && !(Number.isInteger(maxLength) && maxLength > 0)) {
    throw new RangeError('maxLength must be a positive integer');
  }

  // Lowercase before NFD so that characters such as İ also lose their marks.
  const slug = text
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  // Cut at a word boundary when one fits, otherwise cut the first word hard.
  if (slug[maxLength] === '-') {
    return slug.slice(0, maxLength);
  }
  const cut = slug.lastIndexOf('-', maxLength - 1);
  if (cut > 0) {
    return slug.slice(0, cut);
  }
  return slug.slice(0, maxLength);
}

module.exports = { slugify };
