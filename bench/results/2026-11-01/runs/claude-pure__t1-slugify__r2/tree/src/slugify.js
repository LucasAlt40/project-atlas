'use strict';

const DIACRITICS = /[̀-ͯ]/g;
const NON_SLUG_RUN = /[^a-z0-9]+/g;

function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('slugify expects a string');
  }

  const { maxLength } = options;
  if (maxLength !== undefined && !(Number.isInteger(maxLength) && maxLength > 0)) {
    throw new RangeError('maxLength must be a positive integer');
  }

  const slug = text
    .toLowerCase()
    .normalize('NFD')
    .replace(DIACRITICS, '')
    .replace(NON_SLUG_RUN, '-')
    .replace(/^-|-$/g, '');

  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  // The slug is cut back to the last word boundary that fits. If the first
  // word alone is too long, it is cut hard at maxLength.
  const cut = slug.slice(0, maxLength);
  if (slug[maxLength] === '-') {
    return cut;
  }

  const lastHyphen = cut.lastIndexOf('-');
  return lastHyphen === -1 ? cut : cut.slice(0, lastHyphen);
}

module.exports = { slugify };
