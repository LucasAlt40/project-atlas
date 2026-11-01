const COMBINING_MARKS = /[̀-ͯ]/g;

function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
  }

  const { maxLength } = options;
  if (maxLength !== undefined) {
    if (!Number.isInteger(maxLength)) {
      throw new TypeError('options.maxLength must be an integer');
    }
    if (maxLength < 1) {
      throw new RangeError('options.maxLength must be at least 1');
    }
  }

  let slug = text
    .toLowerCase()
    .normalize('NFD')
    .replace(COMBINING_MARKS, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');

  if (maxLength !== undefined && slug.length > maxLength) {
    // Look one character past maxLength so a dash sitting exactly at the boundary is found.
    const lastDash = slug.slice(0, maxLength + 1).lastIndexOf('-');
    slug = lastDash > 0 ? slug.slice(0, lastDash) : slug.slice(0, maxLength);
  }

  return slug;
}

module.exports = { slugify };
