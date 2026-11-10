function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('slugify expects a string');
  }

  let slug = text
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');

  const { maxLength } = options;
  if (maxLength !== undefined && slug.length > maxLength) {
    const head = slug.slice(0, maxLength + 1);
    const cut = head.lastIndexOf('-');
    slug = cut > 0 ? head.slice(0, cut) : slug.slice(0, maxLength);
  }

  return slug;
}

module.exports = { slugify };
