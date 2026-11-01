// Parses CSV text into rows of fields, following RFC 4180.
function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = '';
  let inQuotes = false;
  let atFieldStart = true;
  let rowOpen = false;
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
    } else if (ch === '"' && atFieldStart) {
      inQuotes = true;
      atFieldStart = false;
      rowOpen = true;
    } else if (ch === ',') {
      row.push(field);
      field = '';
      atFieldStart = true;
      rowOpen = true;
    } else if (ch === '\n' || (ch === '\r' && text[i + 1] === '\n')) {
      if (ch === '\r') i++;
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
      atFieldStart = true;
      rowOpen = false;
    } else {
      field += ch;
      atFieldStart = false;
      rowOpen = true;
    }
  }
  if (inQuotes) {
    throw new SyntaxError('Unclosed quoted field in CSV input');
  }
  if (rowOpen) {
    row.push(field);
    rows.push(row);
  }
  return rows;
}

module.exports = { parseCsv };
