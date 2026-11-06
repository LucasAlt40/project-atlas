// Turns text into a URL-friendly slug: "Olá, Mundo!" -> "ola-mundo".
function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('slugify expects a string');
  }

  const { maxLength } = options;
  if (maxLength !== undefined && !(Number.isInteger(maxLength) && maxLength > 0)) {
    throw new RangeError('maxLength must be a positive integer');
  }

  // Split accents off their letters, then drop the combining marks.
  let slug = text
    .normalize('NFD')
    .replace(/\p{Mn}/gu, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  // Keep whole words when the first one fits: the last '-' within maxLength + 1
  // characters marks the end of the longest word prefix that fits.
  const head = slug.slice(0, maxLength + 1);
  const lastDash = head.lastIndexOf('-');
  if (lastDash > 0) {
    return head.slice(0, lastDash);
  }

  // The first word alone is too long, so cut it hard. It has no '-' in range,
  // so the result cannot end with '-'.
  return slug.slice(0, maxLength);
}

module.exports = { slugify };
