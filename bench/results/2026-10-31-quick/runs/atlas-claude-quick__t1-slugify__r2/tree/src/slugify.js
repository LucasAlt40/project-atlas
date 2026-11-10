function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
  }

  const slug = text
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');

  const { maxLength } = options;
  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  // Look one character past the limit so a dash right at maxLength still counts as a word boundary.
  const lastDash = slug.slice(0, maxLength + 1).lastIndexOf('-');
  if (lastDash === -1) {
    return slug.slice(0, maxLength);
  }
  return slug.slice(0, lastDash);
}

module.exports = { slugify };
