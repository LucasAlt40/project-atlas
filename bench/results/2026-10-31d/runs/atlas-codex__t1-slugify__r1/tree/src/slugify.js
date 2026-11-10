function slugify(text, options) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
  }

  const slug = text
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');

  const maxLength = options && options.maxLength;
  if (!Number.isInteger(maxLength) || maxLength <= 0 || slug.length <= maxLength) {
    return slug;
  }

  const words = slug.split('-');
  if (words[0].length > maxLength) {
    return words[0].slice(0, maxLength);
  }

  let result = words[0];
  for (const word of words.slice(1)) {
    const candidate = `${result}-${word}`;
    if (candidate.length > maxLength) {
      break;
    }
    result = candidate;
  }

  return result;
}

module.exports = { slugify };
