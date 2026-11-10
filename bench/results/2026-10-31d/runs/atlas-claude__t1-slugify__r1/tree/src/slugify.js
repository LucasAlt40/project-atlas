function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
  }

  const { maxLength } = options;
  if (maxLength !== undefined) {
    if (typeof maxLength !== 'number') {
      throw new TypeError('options.maxLength must be a number');
    }
    if (!Number.isInteger(maxLength) || maxLength <= 0) {
      throw new RangeError('options.maxLength must be a positive integer');
    }
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

  // The cut lands right before a dash, so the first word ends exactly at maxLength.
  if (slug[maxLength] === '-') {
    return slug.slice(0, maxLength);
  }

  const head = slug.slice(0, maxLength);
  const idx = head.lastIndexOf('-');
  return idx > 0 ? slug.slice(0, idx) : head;
}

module.exports = { slugify };
