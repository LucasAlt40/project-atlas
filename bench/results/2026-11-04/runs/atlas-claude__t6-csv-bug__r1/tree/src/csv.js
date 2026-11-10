// Parses CSV text into rows of fields, following RFC 4180.
function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = '';
  let inQuotes = false;
  // True once the current row has any content, so a trailing newline adds no row.
  let pending = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    pending = true;
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
    } else if (ch === ',') {
      row.push(field);
      field = '';
    } else if (ch === '\r' || ch === '\n') {
      if (ch === '\r' && text[i + 1] === '\n') {
        i++;
      }
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
      pending = false;
    } else {
      field += ch;
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
