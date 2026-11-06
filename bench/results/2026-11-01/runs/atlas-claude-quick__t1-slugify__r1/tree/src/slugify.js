function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('slugify: text must be a string');
  }

  let slug = text
    .normalize('NFD')
    .replace(/\p{Mn}/gu, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  const { maxLength } = options;
  if (maxLength !== undefined && slug.length > maxLength) {
    // Cut at the last hyphen that still fits; hard cut only when the first word is too long.
    const wordEnd = slug.lastIndexOf('-', maxLength);
    slug = wordEnd !== -1 ? slug.slice(0, wordEnd) : slug.slice(0, maxLength);
  }

  return slug;
}

module.exports = { slugify };
