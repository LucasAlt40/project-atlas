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

  // Cut at the last '-' whose prefix still fits, so no word is split.
  const dash = slug.slice(0, maxLength + 1).lastIndexOf('-');
  if (dash !== -1) {
    return slug.slice(0, dash);
  }

  // The first word alone does not fit, so cut it hard.
  return slug.slice(0, maxLength);
}

module.exports = { slugify };
