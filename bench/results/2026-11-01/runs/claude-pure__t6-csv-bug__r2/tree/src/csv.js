// Parses CSV text (RFC 4180) into rows of fields.
function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = '';
  let fieldStart = true;
  let i = 0;

  const endField = () => {
    row.push(field);
    field = '';
    fieldStart = true;
  };
  const endRecord = () => {
    endField();
    rows.push(row);
    row = [];
  };

  while (i < text.length) {
    const ch = text[i];
    if (fieldStart && ch === '"') {
      // Quoted field: commas and line breaks are data, "" is an escaped quote.
      fieldStart = false;
      i++;
      let closed = false;
      while (i < text.length) {
        if (text[i] === '"' && text[i + 1] === '"') {
          field += '"';
          i += 2;
        } else if (text[i] === '"') {
          closed = true;
          i++;
          break;
        } else {
          field += text[i];
          i++;
        }
      }
      if (!closed) {
        throw new SyntaxError('Unterminated quoted field');
      }
    } else if (ch === ',') {
      endField();
      i++;
    } else if (ch === '\n' || (ch === '\r' && text[i + 1] === '\n')) {
      endRecord();
      i += ch === '\r' ? 2 : 1;
    } else {
      field += ch;
      fieldStart = false;
      i++;
    }
  }

  // A final line break already ended its record, so only a partial record is left to add.
  if (row.length > 0 || !fieldStart) {
    endRecord();
  }
  return rows;
}

module.exports = { parseCsv };
