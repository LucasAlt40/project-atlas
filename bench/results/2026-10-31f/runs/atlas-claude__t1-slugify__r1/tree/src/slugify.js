'use strict';

function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('slugify: text must be a string');
  }

  const maxLength = options.maxLength;
  if (maxLength !== undefined && (!Number.isInteger(maxLength) || maxLength <= 0)) {
    throw new RangeError('slugify: maxLength must be a positive integer');
  }

  // Decompose accents, drop the combining marks, then lowercase.
  const slug = text
    .normalize('NFKD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  // The cut lands on a word boundary: keep the prefix as is.
  if (slug[maxLength] === '-') {
    return slug.slice(0, maxLength);
  }

  // Otherwise drop the partial word, or cut hard if the first word does not fit.
  const cut = slug.slice(0, maxLength);
  const dash = cut.lastIndexOf('-');
  return dash === -1 ? cut : cut.slice(0, dash);
}

module.exports = { slugify };
