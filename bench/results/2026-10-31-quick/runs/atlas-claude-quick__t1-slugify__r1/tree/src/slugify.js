function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('slugify: text must be a string');
  }

  const maxLength = options.maxLength;
  if (maxLength !== undefined && (!Number.isInteger(maxLength) || maxLength <= 0)) {
    throw new RangeError('slugify: maxLength must be a positive integer');
  }

  const slug = text
    .normalize('NFD')
    .replace(/\p{M}/gu, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  // Look one character past maxLength so a hyphen right after the limit still counts as a word boundary.
  const head = slug.slice(0, maxLength + 1);
  const lastHyphen = head.lastIndexOf('-');
  if (lastHyphen !== -1) {
    return head.slice(0, lastHyphen);
  }

  // The first word alone does not fit, so cut it hard.
  return slug.slice(0, maxLength);
}

module.exports = { slugify };
