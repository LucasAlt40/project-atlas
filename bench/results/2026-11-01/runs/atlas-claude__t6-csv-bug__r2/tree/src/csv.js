// Parses CSV text into rows of fields (RFC 4180).
// Quoted fields may contain commas, line breaks and doubled quotes ("").
// Rows end with \n or \r\n; a trailing line break does not add an empty row.
function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = '';
  let inQuotes = false;
  let fieldStart = true;
  let i = 0;
  while (i < text.length) {
    const ch = text[i];
    if (inQuotes) {
      if (ch === '"' && text[i + 1] === '"') {
        field += '"';
        i += 2;
        continue;
      }
      if (ch === '"') {
        inQuotes = false;
      } else {
        field += ch;
      }
    } else if (ch === '"' && fieldStart) {
      inQuotes = true;
    } else if (ch === ',') {
      row.push(field);
      field = '';
      fieldStart = true;
      i++;
      continue;
    } else if (ch === '\n' || (ch === '\r' && text[i + 1] === '\n')) {
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
      fieldStart = true;
      i += ch === '\r' ? 2 : 1;
      continue;
    } else {
      field += ch;
    }
    fieldStart = false;
    i++;
  }
  if (inQuotes) {
    throw new SyntaxError('Unterminated quoted field');
  }
  if (!fieldStart || row.length > 0) {
    row.push(field);
    rows.push(row);
  }
  return rows;
}

module.exports = { parseCsv };
