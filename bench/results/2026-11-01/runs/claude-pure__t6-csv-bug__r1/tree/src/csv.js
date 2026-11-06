// Parses CSV text (RFC 4180) into rows of fields.
function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = '';
  let inQuotes = false;
  let hasContent = false; // the current row has started but not ended
  for (let i = 0; i < text.length; i++) {
    const char = text[i];
    if (inQuotes) {
      if (char !== '"') {
        field += char;
      } else if (text[i + 1] === '"') {
        field += '"';
        i++;
      } else {
        inQuotes = false;
      }
    } else if (char === '"' && field === '') {
      inQuotes = true;
      hasContent = true;
    } else if (char === ',') {
      row.push(field);
      field = '';
      hasContent = true;
    } else if (char === '\n' || (char === '\r' && text[i + 1] === '\n')) {
      if (char === '\r') i++;
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
      hasContent = false;
    } else {
      field += char;
      hasContent = true;
    }
  }
  if (inQuotes) throw new SyntaxError('Unclosed quoted field');
  if (hasContent) {
    row.push(field);
    rows.push(row);
  }
  return rows;
}

module.exports = { parseCsv };
