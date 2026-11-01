function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('slugify expects a string');
  }

  const maxLength = options.maxLength;
  if (maxLength !== undefined && !(Number.isInteger(maxLength) && maxLength > 0)) {
    throw new RangeError('options.maxLength must be a positive integer');
  }

  // Decompose accents, drop the combining marks, then turn every run of
  // non a-z/0-9 characters into a single hyphen.
  const slug = text
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  // Keep one extra character so a hyphen right at the limit is still seen.
  const head = slug.slice(0, maxLength + 1);
  const lastHyphen = head.lastIndexOf('-');
  if (lastHyphen > 0) {
    return head.slice(0, lastHyphen);
  }

  // The first word alone does not fit, so cut it hard.
  return slug.slice(0, maxLength);
}

module.exports = { slugify };
