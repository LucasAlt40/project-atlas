// Parses CSV text into rows of fields (RFC 4180).
function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = '';
  let inQuotes = false;
  // True once the current row has consumed any character, so a final
  // row without a trailing newline is kept and a trailing newline adds nothing.
  let rowOpen = false;
  // True until the current field has consumed a character, so a quote only
  // opens a quoted field at the start of a field (a quote mid-field is literal).
  let fieldStart = true;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (inQuotes) {
      if (ch === '"') {
        if (text[i + 1] === '"') {
          field += '"';
          i++;
        } else {
          inQuotes = false;
        }
      } else {
        field += ch;
      }
    } else if (ch === '"' && fieldStart) {
      inQuotes = true;
      rowOpen = true;
      fieldStart = false;
    } else if (ch === ',') {
      row.push(field);
      field = '';
      rowOpen = true;
      fieldStart = true;
    } else if (ch === '\r' && (i + 1 === text.length || text[i + 1] === '\n')) {
      // Part of a CRLF (or trailing CR) line ending; the '\n' ends the row.
      continue;
    } else if (ch === '\n') {
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
      rowOpen = false;
      fieldStart = true;
    } else {
      field += ch;
      rowOpen = true;
      fieldStart = false;
    }
  }
  if (inQuotes) {
    throw new SyntaxError('Unterminated quoted field');
  }
  if (rowOpen) {
    row.push(field);
    rows.push(row);
  }
  return rows;
}

module.exports = { parseCsv };
