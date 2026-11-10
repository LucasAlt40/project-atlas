// Turns text into a lowercase slug made only of a-z, 0-9 and single '-' separators.
function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
  }

  const { maxLength } = options;
  const slug = text
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  if (maxLength === undefined) {
    return slug;
  }
  if (!Number.isInteger(maxLength)) {
    throw new TypeError('options.maxLength must be an integer');
  }
  if (maxLength < 1) {
    throw new RangeError('options.maxLength must be at least 1');
  }
  if (slug.length <= maxLength) {
    return slug;
  }

  // Keep whole words: cut at the last '-' that still fits within maxLength.
  const head = slug.slice(0, maxLength + 1);
  const lastDash = head.lastIndexOf('-');
  if (lastDash !== -1) {
    return head.slice(0, lastDash);
  }

  // The first word alone does not fit: cut it hard.
  return slug.slice(0, maxLength);
}

module.exports = { slugify };
