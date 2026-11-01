function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('slugify: text must be a string');
  }

  const { maxLength } = options;
  if (maxLength !== undefined && !(Number.isInteger(maxLength) && maxLength > 0)) {
    throw new RangeError('slugify: maxLength must be a positive integer');
  }

  // Lowercase, split accents off their base letters, then drop the combining marks
  const base = text.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '');

  // Collapse every run of non a-z0-9 characters into one hyphen, then trim the ends
  const slug = base.replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '');

  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  // Keep the whole word if the cut lands exactly on a hyphen
  if (slug[maxLength] === '-') {
    return slug.slice(0, maxLength);
  }

  // Otherwise cut at the last hyphen that fits, so no word is split
  const head = slug.slice(0, maxLength);
  const lastHyphen = head.lastIndexOf('-');
  return lastHyphen > 0 ? head.slice(0, lastHyphen) : head;
}

module.exports = { slugify };
