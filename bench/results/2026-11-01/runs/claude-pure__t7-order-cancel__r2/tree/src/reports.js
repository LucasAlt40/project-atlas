// Sales figures over every order that was not cancelled.
function createReports({ orders }) {
  const active = () => orders.all().filter((o) => o.status !== 'cancelled');
  return {
    revenue() {
      return active().reduce((sum, o) => sum + o.total, 0);
    },
    unitsSold(sku) {
      return active()
        .flatMap((o) => o.items)
        .filter((i) => i.sku === sku)
        .reduce((sum, i) => sum + i.qty, 0);
    },
  };
}

module.exports = { createReports };
