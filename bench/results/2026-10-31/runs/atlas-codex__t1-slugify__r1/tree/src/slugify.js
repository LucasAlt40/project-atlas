function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
  }

  const slug = text
    .normalize('NFD')
    .replace(/\p{M}/gu, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  const { maxLength } = options;
  if (!Number.isInteger(maxLength) || maxLength <= 0 || slug.length <= maxLength) {
    return slug;
  }

  const prefix = slug.slice(0, maxLength);
  if (slug[maxLength] === '-') {
    return prefix;
  }

  const lastSeparator = prefix.lastIndexOf('-');

  return lastSeparator > 0 ? prefix.slice(0, lastSeparator) : prefix;
}

module.exports = { slugify };
