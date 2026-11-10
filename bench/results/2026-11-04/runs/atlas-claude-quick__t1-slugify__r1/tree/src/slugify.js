'use strict';

const COMBINING_MARKS = /[̀-ͯ]/g;
const NON_SLUG_RUNS = /[^a-z0-9]+/g;

function validateMaxLength(maxLength) {
  if (maxLength !== undefined && (!Number.isInteger(maxLength) || maxLength <= 0)) {
    throw new TypeError('options.maxLength must be a positive integer');
  }
}

function slugify(text, options = {}) {
  if (typeof text !== 'string') {
    throw new TypeError('text must be a string');
  }

  const { maxLength } = options;
  validateMaxLength(maxLength);

  // Strip diacritics before lowercasing so decomposed letters keep their base form.
  const slug = text
    .normalize('NFD')
    .replace(COMBINING_MARKS, '')
    .toLowerCase()
    .replace(NON_SLUG_RUNS, '-')
    .replace(/^-|-$/g, '');

  if (maxLength === undefined || slug.length <= maxLength) {
    return slug;
  }

  // Prefer cutting at the last word boundary that fits; otherwise hard-cut the first word.
  const boundary = slug.slice(0, maxLength + 1).lastIndexOf('-');
  return boundary > 0 ? slug.slice(0, boundary) : slug.slice(0, maxLength);
}

module.exports = { slugify };
