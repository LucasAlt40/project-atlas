function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
  }

  const { maxLength } = options;
  if (maxLength !== undefined && !(Number.isInteger(maxLength) && maxLength > 0)) {
    throw new RangeError('maxLength must be a positive integer');
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

  const head = slug.slice(0, maxLength + 1);
  const cut = head.lastIndexOf('-');
  if (cut > 0) {
    return head.slice(0, cut);
  }

  return slug.slice(0, maxLength);
}

module.exports = { slugify };
