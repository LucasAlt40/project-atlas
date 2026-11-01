function slugify(text, options) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
  }

  const slug = text
    .normalize('NFD')
    .replace(/\p{M}/gu, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');

  const maxLength = options && options.maxLength;
  if (!Number.isInteger(maxLength) || maxLength <= 0 || slug.length <= maxLength) {
    return slug;
  }

  const truncated = slug.slice(0, maxLength);
  if (slug[maxLength] === '-') {
    return truncated;
  }

  const lastSeparator = truncated.lastIndexOf('-');
  return lastSeparator > 0 ? truncated.slice(0, lastSeparator) : truncated;
}

module.exports = { slugify };
