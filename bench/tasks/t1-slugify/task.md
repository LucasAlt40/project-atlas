Create the file src/slugify.js, a CommonJS module that exports a function slugify(text, options), exactly as: module.exports = { slugify }.

Rules:

1. The result is lowercase.
2. Accents and diacritics are removed (é -> e, ç -> c, ã -> a, ü -> u).
3. Every run of characters that are not a-z or 0-9 becomes a single '-'.
4. The result never starts or ends with '-'.
5. options.maxLength (optional, a positive integer): the result is at most maxLength characters. Cut at the last '-' that fits, so no word is split and the result does not end with '-'. If even the first word does not fit, cut it hard at maxLength.
6. If text is not a string, throw a TypeError.

Examples: slugify(' Olá, Mundo! ') === 'ola-mundo'; slugify('Ação & Reação') === 'acao-reacao'; slugify('hello big world', { maxLength: 9 }) === 'hello-big'; slugify('hello big world', { maxLength: 8 }) === 'hello'; slugify('supercalifragilistic', { maxLength: 5 }) === 'super'.

Also add tests in test/slugify.test.js using node:test and node:assert. Touch no other file.
