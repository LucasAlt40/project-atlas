// Sales figures over every order that has not been cancelled.
function createReports({ orders }) {
  const sales = () => orders.all().filter((o) => o.status !== 'cancelled');
  return {
    revenue() {
      return sales().reduce((sum, o) => sum + o.total, 0);
    },
    unitsSold(sku) {
      return sales()
        .flatMap((o) => o.items)
        .filter((i) => i.sku === sku)
        .reduce((sum, i) => sum + i.qty, 0);
    },
  };
}

module.exports = { createReports };
