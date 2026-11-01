// Parses CSV text into rows of fields (RFC 4180).
function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = '';
  let fieldStart = true;
  let quoted = false;
  let i = 0;
  while (i < text.length) {
    const ch = text[i];
    if (quoted) {
      if (ch === '"' && text[i + 1] === '"') {
        field += '"';
        i += 2;
        continue;
      }
      if (ch === '"') {
        quoted = false;
      } else {
        field += ch;
      }
      i++;
      continue;
    }
    if (ch === '"' && fieldStart) {
      quoted = true;
      fieldStart = false;
    } else if (ch === ',') {
      row.push(field);
      field = '';
      fieldStart = true;
    } else if (ch === '\r' && text[i + 1] === '\n') {
      // CRLF: the '\n' ends the row on the next step.
    } else if (ch === '\n') {
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
      fieldStart = true;
    } else {
      field += ch;
      fieldStart = false;
    }
    i++;
  }
  if (quoted) {
    throw new SyntaxError('Unclosed quoted field');
  }
  if (!fieldStart || row.length > 0) {
    row.push(field);
    rows.push(row);
  }
  return rows;
}

module.exports = { parseCsv };
