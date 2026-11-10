function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('slugify: text must be a string');
  }

  // Decompose accented letters and drop the combining marks (é -> e, ç -> c).
  const slug = text
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  const { maxLength } = options;
  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  // The word ends exactly at maxLength, so the whole prefix fits.
  if (slug[maxLength] === '-') {
    return slug.slice(0, maxLength);
  }

  // Cut at the last separator that fits, so no word is split.
  const head = slug.slice(0, maxLength);
  const lastDash = head.lastIndexOf('-');
  if (lastDash !== -1) {
    return head.slice(0, lastDash);
  }

  // The first word alone does not fit, so cut it hard.
  return head;
}

module.exports = { slugify };
