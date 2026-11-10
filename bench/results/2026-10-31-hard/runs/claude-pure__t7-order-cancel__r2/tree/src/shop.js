const { createBus } = require('./events.js');
const { createInventory } = require('./inventory.js');
const { createOrders } = require('./orders.js');
const { createReports } = require('./reports.js');

/** Everything wired together. */
function createShop({ stock = {}, prices = {} } = {}) {
  const bus = createBus();
  const inventory = createInventory(stock);
  const orders = createOrders({ inventory, bus, prices });
  const reports = createReports({ orders });
  return { bus, inventory, orders, reports };
}

module.exports = { createShop };
