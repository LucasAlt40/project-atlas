function slugify(text, options) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
  }

  const slug = text
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');

  const maxLength = options && options.maxLength;
  if (!Number.isInteger(maxLength) || maxLength <= 0 || slug.length <= maxLength) {
    return slug;
  }

  const lastHyphen = slug.lastIndexOf('-', maxLength);
  return lastHyphen === -1 ? slug.slice(0, maxLength) : slug.slice(0, lastHyphen);
}

module.exports = { slugify };
