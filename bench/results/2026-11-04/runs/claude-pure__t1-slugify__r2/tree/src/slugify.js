const DIACRITICS = /[̀-ͯ]/g;
const NON_SLUG_RUN = /[^a-z0-9]+/g;
const EDGE_DASHES = /^-+|-+$/g;

function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('slugify: text must be a string');
  }

  const { maxLength } = options;
  if (maxLength !== undefined && !(Number.isInteger(maxLength) && maxLength > 0)) {
    throw new RangeError('slugify: maxLength must be a positive integer');
  }

  const slug = text
    .toLowerCase()
    .normalize('NFD')
    .replace(DIACRITICS, '')
    .replace(NON_SLUG_RUN, '-')
    .replace(EDGE_DASHES, '');

  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  // A dash at index maxLength still lets the word before it fit, so look one past the limit.
  const lastDash = slug.slice(0, maxLength + 1).lastIndexOf('-');
  if (lastDash > 0) {
    return slug.slice(0, lastDash);
  }

  return slug.slice(0, maxLength);
}

module.exports = { slugify };
