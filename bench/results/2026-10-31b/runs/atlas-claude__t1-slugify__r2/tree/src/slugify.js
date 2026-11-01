const COMBINING_MARKS = /[̀-ͯ]/g;
const NON_SLUG_RUN = /[^a-z0-9]+/g;

function slugify(text, options) {
  if (typeof text !== 'string') {
    throw new TypeError('slugify: text must be a string');
  }

  const { maxLength } = options || {};
  if (maxLength !== undefined && !(Number.isInteger(maxLength) && maxLength > 0)) {
    throw new RangeError('slugify: maxLength must be a positive integer');
  }

  const slug = text
    .toLowerCase()
    .normalize('NFD')
    .replace(COMBINING_MARKS, '')
    .replace(NON_SLUG_RUN, '-')
    .replace(/^-|-$/g, '');

  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  // Cut at a dash that falls exactly on the limit, otherwise at the last dash that fits.
  if (slug[maxLength] === '-') {
    return slug.slice(0, maxLength);
  }

  const lastDash = slug.lastIndexOf('-', maxLength - 1);
  return lastDash > 0 ? slug.slice(0, lastDash) : slug.slice(0, maxLength);
}

module.exports = { slugify };
