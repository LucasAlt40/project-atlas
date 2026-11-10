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
    if (slug[maxLength] === '-') {
      slug = slug.slice(0, maxLength);
    } else {
      // Cut at the last word boundary that fits; hard cut if the first word is too long.
      const cut = slug.lastIndexOf('-', maxLength);
      slug = cut !== -1 ? slug.slice(0, cut) : slug.slice(0, maxLength);
    }
  }

  return slug;
}

module.exports = { slugify };
