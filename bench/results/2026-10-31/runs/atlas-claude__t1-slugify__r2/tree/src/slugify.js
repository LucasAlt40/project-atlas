'use strict';

function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('slugify: text must be a string');
  }

  const { maxLength } = options;
  if (maxLength !== undefined && (!Number.isInteger(maxLength) || maxLength <= 0)) {
    throw new RangeError('slugify: maxLength must be a positive integer');
  }

  const slug = text
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  // Look one character past maxLength so a '-' at index maxLength (end of a fitting word) is found.
  const lastDash = slug.slice(0, maxLength + 1).lastIndexOf('-');
  return lastDash !== -1 ? slug.slice(0, lastDash) : slug.slice(0, maxLength);
}

module.exports = { slugify };
