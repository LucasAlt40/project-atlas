// Parses CSV text into rows of fields.
function parseCsv(text) {
  const rows = [];
  for (const line of text.split('\n')) {
    const fields = line.split(',').map((field) => {
      if (field.startsWith('"') && field.endsWith('"')) {
        return field.slice(1, -1);
      }
      return field;
    });
    rows.push(fields);
  }
  return rows;
}

module.exports = { parseCsv };
