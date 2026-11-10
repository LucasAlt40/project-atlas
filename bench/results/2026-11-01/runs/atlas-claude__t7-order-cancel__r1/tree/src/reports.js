// Sales figures over every order that was not cancelled.
function createReports({ orders }) {
  const live = () => orders.all().filter((o) => o.status !== 'cancelled');
  return {
    revenue() {
      return live().reduce((sum, o) => sum + o.total, 0);
    },
    unitsSold(sku) {
      return live()
        .flatMap((o) => o.items)
        .filter((i) => i.sku === sku)
        .reduce((sum, i) => sum + i.qty, 0);
    },
  };
}

module.exports = { createReports };
