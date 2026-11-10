function slugify(text, options) {
  if (typeof text !== 'string') {
    throw new TypeError('slugify: text must be a string');
  }

  const maxLength = (options || {}).maxLength;
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

  const cut = slug.slice(0, maxLength);
  if (slug[maxLength] === '-') {
    return cut;
  }

  const lastDash = cut.lastIndexOf('-');
  if (lastDash > 0) {
    return cut.slice(0, lastDash);
  }

  return cut;
}

module.exports = { slugify };
