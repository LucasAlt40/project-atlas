function slugify(text, options) {
  if (typeof text !== 'string') {
    throw new TypeError('slugify: text must be a string');
  }

  const { maxLength } = options ?? {};
  if (maxLength !== undefined && (!Number.isInteger(maxLength) || maxLength < 1)) {
    throw new RangeError('slugify: maxLength must be a positive integer');
  }

  const result = text
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  if (maxLength === undefined || result.length <= maxLength) {
    return result;
  }

  // Look one character past maxLength so a dash right at the boundary still counts.
  const window = result.slice(0, maxLength + 1);
  const cut = window.lastIndexOf('-');
  if (cut > 0) {
    return window.slice(0, cut);
  }

  // The first word alone does not fit, so cut it hard.
  return result.slice(0, maxLength);
}

module.exports = { slugify };
