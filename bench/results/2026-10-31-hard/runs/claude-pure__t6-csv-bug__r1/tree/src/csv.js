// Parses CSV text into rows of fields (RFC 4180).
function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = '';
  let inQuotes = false;
  // True at the start of a field, where a quote opens a quoted field.
  let fieldStart = true;
  // True once the current row has any content, so a trailing newline adds no row.
  let inRow = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    inRow = true;
    if (inQuotes) {
      if (ch === '"' && text[i + 1] === '"') {
        field += '"';
        i++;
      } else if (ch === '"') {
        inQuotes = false;
      } else {
        field += ch;
      }
    } else if (ch === '"' && fieldStart) {
      inQuotes = true;
      fieldStart = false;
    } else if (ch === ',') {
      row.push(field);
      field = '';
      fieldStart = true;
    } else if (ch === '\r' && text[i + 1] === '\n') {
      // The \n that follows ends the row.
    } else if (ch === '\n') {
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
      fieldStart = true;
      inRow = false;
    } else {
      field += ch;
      fieldStart = false;
    }
  }
  if (inQuotes) {
    throw new SyntaxError('Unterminated quoted field');
  }
  if (inRow) {
    row.push(field);
    rows.push(row);
  }
  return rows;
}

module.exports = { parseCsv };
