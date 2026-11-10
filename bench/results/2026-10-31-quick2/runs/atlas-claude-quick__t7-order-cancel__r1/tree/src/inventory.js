// Stock per SKU.
function createInventory(initial = {}) {
  const stock = new Map(Object.entries(initial));
  return {
    quantity(sku) {
      return stock.get(sku) ?? 0;
    },
    take(sku, qty) {
      const have = stock.get(sku) ?? 0;
      if (qty > have) throw new Error(`not enough ${sku}`);
      stock.set(sku, have - qty);
    },
    put(sku, qty) {
      stock.set(sku, (stock.get(sku) ?? 0) + qty);
    },
  };
}

module.exports = { createInventory };
