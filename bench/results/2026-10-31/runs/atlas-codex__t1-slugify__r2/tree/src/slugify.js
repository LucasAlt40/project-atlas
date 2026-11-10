function slugify(text, options) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
  }

  const normalized = text
    .normalize('NFD')
    .replace(/\p{M}/gu, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');

  const maxLength = options && options.maxLength;
  if (!Number.isInteger(maxLength) || maxLength <= 0 || normalized.length <= maxLength) {
    return normalized;
  }

  const prefix = normalized.slice(0, maxLength);
  if (normalized[maxLength] === '-') {
    return prefix;
  }

  const lastHyphen = prefix.lastIndexOf('-');
  return lastHyphen === -1 ? prefix : prefix.slice(0, lastHyphen);
}

module.exports = { slugify };
