function slugify(text, options) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
  }

  const slug = text
    .toLowerCase()
    .normalize('NFD')
    .replace(/\p{M}+/gu, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  const maxLength = options && options.maxLength;
  if (!Number.isInteger(maxLength) || maxLength <= 0 || slug.length <= maxLength) {
    return slug;
  }

  const prefix = slug.slice(0, maxLength);
  if (slug[maxLength] === '-') {
    return prefix;
  }

  const lastHyphen = prefix.lastIndexOf('-');

  return lastHyphen > 0 ? prefix.slice(0, lastHyphen) : prefix;
}

module.exports = { slugify };
