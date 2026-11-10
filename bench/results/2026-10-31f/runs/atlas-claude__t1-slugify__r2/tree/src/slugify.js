function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('slugify: text must be a string');
  }

  const { maxLength } = options;
  if (maxLength !== undefined && !(Number.isInteger(maxLength) && maxLength > 0)) {
    throw new RangeError('slugify: maxLength must be a positive integer');
  }

  const slug = text
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  // Keep the separator at index maxLength in view so a cut right before it is allowed.
  const lastFittingDash = slug.slice(0, maxLength + 1).lastIndexOf('-');
  if (lastFittingDash > 0) {
    return slug.slice(0, lastFittingDash);
  }

  // The first word alone does not fit, so cut it hard.
  return slug.slice(0, maxLength);
}

module.exports = { slugify };
