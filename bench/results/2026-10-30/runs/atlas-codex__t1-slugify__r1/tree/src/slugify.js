'use strict';

function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
  }

  const slug = text
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  const { maxLength } = options;
  if (Number.isInteger(maxLength) && maxLength > 0 && slug.length > maxLength) {
    const prefix = slug.slice(0, maxLength);

    if (!prefix.endsWith('-')) {
      return prefix;
    }

    const lastHyphen = prefix.lastIndexOf('-');
    return prefix.slice(0, lastHyphen);
  }

  return slug;
}

module.exports = { slugify };
