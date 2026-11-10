// Parses CSV text (RFC 4180) into rows of fields.
function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = '';
  let inQuotes = false;
  let pending = false; // true once the current row has any characters
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (inQuotes) {
      if (ch !== '"') {
        field += ch;
      } else if (text[i + 1] === '"') {
        field += '"';
        i++;
      } else {
        inQuotes = false;
      }
    } else if (ch === '"' && field === '') {
      inQuotes = true;
      pending = true;
    } else if (ch === ',') {
      row.push(field);
      field = '';
      pending = true;
    } else if (ch === '\n' || (ch === '\r' && text[i + 1] === '\n')) {
      if (ch === '\r') i++;
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
      pending = false;
    } else {
      field += ch;
      pending = true;
    }
  }
  if (inQuotes) {
    throw new SyntaxError('Unclosed quoted field');
  }
  if (pending) {
    row.push(field);
    rows.push(row);
  }
  return rows;
}

module.exports = { parseCsv };
