// Parses CSV text into rows of fields, following RFC 4180.
function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = '';
  let quoted = false;
  let pending = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    pending = true;
    if (quoted) {
      if (ch !== '"') {
        field += ch;
      } else if (text[i + 1] === '"') {
        field += '"';
        i++;
      } else {
        quoted = false;
      }
    } else if (ch === '"' && field === '') {
      quoted = true;
    } else if (ch === ',') {
      row.push(field);
      field = '';
    } else if (ch === '\n' || (ch === '\r' && text[i + 1] === '\n')) {
      if (ch === '\r') i++;
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
      pending = false;
    } else {
      field += ch;
    }
  }
  if (quoted) {
    throw new SyntaxError('Unterminated quoted field');
  }
  if (pending) {
    row.push(field);
    rows.push(row);
  }
  return rows;
}

module.exports = { parseCsv };
