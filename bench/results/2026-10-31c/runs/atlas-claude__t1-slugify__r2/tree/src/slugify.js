function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
  }

  const { maxLength } = options;
  if (maxLength !== undefined && (!Number.isInteger(maxLength) || maxLength < 1)) {
    throw new RangeError('maxLength must be a positive integer');
  }

  const slug = text
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  // The head fits exactly when the next character is a separator.
  if (slug[maxLength] === '-') {
    return slug.slice(0, maxLength);
  }

  // Cut at the last separator that fits, so no word is split.
  const cut = slug.lastIndexOf('-', maxLength);
  if (cut > 0) {
    return slug.slice(0, cut);
  }

  // The first word alone is too long: cut it hard.
  return slug.slice(0, maxLength);
}

module.exports = { slugify };
