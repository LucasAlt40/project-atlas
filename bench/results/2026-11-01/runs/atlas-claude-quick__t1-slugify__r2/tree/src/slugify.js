function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
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

  // Look one character past maxLength so a '-' right at the limit still counts as a word boundary.
  const head = slug.slice(0, maxLength + 1);
  const lastDash = head.lastIndexOf('-');
  if (lastDash > 0) {
    return head.slice(0, lastDash);
  }

  // The first word alone does not fit, so cut it hard.
  return slug.slice(0, maxLength);
}

module.exports = { slugify };
