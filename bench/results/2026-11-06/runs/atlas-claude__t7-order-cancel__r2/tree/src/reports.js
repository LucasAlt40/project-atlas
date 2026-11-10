// Sales figures over every order.
function createReports({ orders }) {
  return {
    revenue() {
      return orders
        .all()
        .filter((o) => o.status !== 'cancelled')
        .reduce((sum, o) => sum + o.total, 0);
    },
    unitsSold(sku) {
      return orders
        .all()
        .filter((o) => o.status !== 'cancelled')
        .flatMap((o) => o.items)
        .filter((i) => i.sku === sku)
        .reduce((sum, i) => sum + i.qty, 0);
    },
  };
}

module.exports = { createReports };
