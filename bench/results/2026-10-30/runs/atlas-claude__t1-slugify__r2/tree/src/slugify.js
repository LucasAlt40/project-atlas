// Turns text into a URL-friendly slug: lowercase a-z and 0-9 words joined by '-'.
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
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  // Keep whole words only: if the cut falls right before a '-', the word ends exactly at the limit.
  const cut = slug.slice(0, maxLength);
  if (slug[maxLength] === '-') {
    return cut;
  }

  const lastDash = cut.lastIndexOf('-');
  return lastDash === -1 ? cut : cut.slice(0, lastDash);
}

module.exports = { slugify };
