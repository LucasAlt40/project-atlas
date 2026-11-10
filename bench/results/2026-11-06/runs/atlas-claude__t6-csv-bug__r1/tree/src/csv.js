// Parses CSV text into rows of fields (RFC 4180).
// Scans character by character so that quoted fields may contain commas,
// line breaks and doubled quotes. Records end at LF or CRLF.
function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = '';
  let inQuotes = false;
  // True once the current record has any content, so that a final
  // newline does not produce an extra empty row.
  let recordStarted = false;
  // True only at the first character of a field. A quote opens a quoted
  // section only there; elsewhere it is literal data (e.g. 5" screen).
  let atFieldStart = true;
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
      recordStarted = true;
    } else if (ch === ',') {
      row.push(field);
      field = '';
      recordStarted = true;
      atFieldStart = true;
    } else if (ch === '\n' || (ch === '\r' && text[i + 1] === '\n')) {
      if (ch === '\r') {
        i++;
      }
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
      recordStarted = false;
      atFieldStart = true;
    } else {
      field += ch;
      recordStarted = true;
      atFieldStart = false;
    }
  }
  if (inQuotes) {
    throw new SyntaxError('CSV: unterminated quoted field');
  }
  if (recordStarted) {
    row.push(field);
    rows.push(row);
  }
  return rows;
}

module.exports = { parseCsv };
