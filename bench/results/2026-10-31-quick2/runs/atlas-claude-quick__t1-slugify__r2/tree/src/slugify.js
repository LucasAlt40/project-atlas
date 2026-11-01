function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('slugify: text must be a string');
  }

  const { maxLength } = options;
  if (maxLength !== undefined && !(Number.isInteger(maxLength) && maxLength > 0)) {
    throw new RangeError('slugify: maxLength must be a positive integer');
  }

  let slug = text
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  if (maxLength !== undefined && slug.length > maxLength) {
    // Keep a window one char longer than maxLength so a '-' sitting exactly at
    // index maxLength (the word ends right at the limit) still counts as a cut point.
    const window = slug.slice(0, maxLength + 1);
    const cut = window.lastIndexOf('-');
    slug = cut === -1 ? slug.slice(0, maxLength) : slug.slice(0, cut);
  }

  return slug;
}

module.exports = { slugify };
