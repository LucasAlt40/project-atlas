function slugify(text, options) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
  }

  let slug = text
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');

  const maxLength = options && options.maxLength;
  if (Number.isInteger(maxLength) && maxLength > 0 && slug.length > maxLength) {
    const words = slug.split('-');
    if (words[0].length > maxLength) {
      slug = words[0].slice(0, maxLength);
    } else {
      slug = words[0];
      for (const word of words.slice(1)) {
        if (slug.length + 1 + word.length > maxLength) {
          break;
        }
        slug += `-${word}`;
      }
    }
  }

  return slug;
}

module.exports = { slugify };
