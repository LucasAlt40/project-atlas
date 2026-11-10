// Turns text into a URL-friendly slug: lowercase a-z and 0-9 words joined by '-'.
function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
  }

  const { maxLength } = options;
  if (maxLength !== undefined) {
    if (typeof maxLength !== 'number') {
      throw new TypeError('maxLength must be a number');
    }
    if (!Number.isInteger(maxLength) || maxLength <= 0) {
      throw new RangeError('maxLength must be a positive integer');
    }
  }

  const slug = text
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');

  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  // Look one character past the limit so a dash exactly at maxLength still counts as a boundary.
  const head = slug.slice(0, maxLength + 1);
  const dash = head.lastIndexOf('-');
  if (dash > 0) {
    return head.slice(0, dash);
  }

  // The first word alone does not fit, so cut it hard.
  return slug.slice(0, maxLength);
}

module.exports = { slugify };
