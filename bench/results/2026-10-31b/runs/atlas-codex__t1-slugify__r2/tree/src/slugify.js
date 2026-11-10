function slugify(text, options) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
  }

  const normalized = text
    .normalize('NFD')
    .replace(/\p{M}/gu, '')
    .toLowerCase();
  const slug = normalized
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');

  const maxLength = options && options.maxLength;
  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  const firstSeparator = slug.indexOf('-');
  if (firstSeparator === -1 || firstSeparator > maxLength) {
    return slug.slice(0, maxLength);
  }

  const words = slug.split('-');
  let result = words[0];
  for (let index = 1; index < words.length; index += 1) {
    const candidate = `${result}-${words[index]}`;
    if (candidate.length > maxLength) {
      break;
    }
    result = candidate;
  }

  return result;
}

module.exports = { slugify };
