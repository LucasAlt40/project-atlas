function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('slugify: text must be a string');
  }

  const slug = text
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  const { maxLength } = options;
  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  const lastDash = slug.lastIndexOf('-', maxLength);
  if (lastDash > 0) {
    return slug.slice(0, lastDash);
  }

  return slug.slice(0, maxLength);
}

module.exports = { slugify };
