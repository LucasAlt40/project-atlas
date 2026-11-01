const { test } = require('node:test');
const assert = require('node:assert/strict');
const { createOrders } = require('../src/orders');
const { createInventory } = require('../src/inventory');
const { createBus } = require('../src/events');
const { createReports } = require('../src/reports');

function setup() {
  const inventory = createInventory({ apple: 10, pear: 5 });
  const bus = createBus();
  const orders = createOrders({ inventory, bus, prices: { apple: 2, pear: 3 } });
  const reports = createReports({ orders });
  const events = [];
  bus.on('order.cancelled', (payload) => events.push(payload));
  return { inventory, orders, reports, events };
}

test('cancel returns the cancelled order with its reason', () => {
  const { orders } = setup();
  const placed = orders.place([{ sku: 'apple', qty: 2 }]);
  const cancelled = orders.cancel(placed.id, 'changed mind');
  assert.equal(cancelled.id, placed.id);
  assert.equal(cancelled.status, 'cancelled');
  assert.equal(cancelled.reason, 'changed mind');
  assert.deepEqual(orders.get(placed.id), cancelled);
});

test('cancel puts the items back into stock', () => {
  const { inventory, orders } = setup();
  const placed = orders.place([
    { sku: 'apple', qty: 4 },
    { sku: 'pear', qty: 2 },
  ]);
  assert.equal(inventory.quantity('apple'), 6);
  assert.equal(inventory.quantity('pear'), 3);
  orders.cancel(placed.id, 'duplicate');
  assert.equal(inventory.quantity('apple'), 10);
  assert.equal(inventory.quantity('pear'), 5);
});

test('cancel emits one order.cancelled event with orderId and reason', () => {
  const { orders, events } = setup();
  const placed = orders.place([{ sku: 'apple', qty: 1 }]);
  orders.cancel(placed.id, 'out of stock');
  assert.deepEqual(events, [{ orderId: placed.id, reason: 'out of stock' }]);
});

test('cancelling a missing order throws not found', () => {
  const { orders, inventory, events } = setup();
  assert.throws(() => orders.cancel(999, 'nope'), (err) => {
    assert.ok(err instanceof Error);
    assert.match(err.message, /not found/);
    return true;
  });
  assert.equal(inventory.quantity('apple'), 10);
  assert.equal(events.length, 0);
});

test('cancelling twice throws already cancelled and changes nothing', () => {
  const { orders, inventory, events } = setup();
  const placed = orders.place([{ sku: 'apple', qty: 3 }]);
  orders.cancel(placed.id, 'first');
  assert.equal(inventory.quantity('apple'), 10);
  assert.throws(() => orders.cancel(placed.id, 'second'), (err) => {
    assert.ok(err instanceof Error);
    assert.match(err.message, /already cancelled/);
    return true;
  });
  assert.equal(inventory.quantity('apple'), 10);
  assert.equal(events.length, 1);
  assert.equal(orders.get(placed.id).reason, 'first');
});

test('an empty or non-string reason throws TypeError and changes nothing', () => {
  const { orders, inventory, events } = setup();
  const placed = orders.place([{ sku: 'apple', qty: 2 }]);
  for (const reason of ['', '   ', undefined, null, 42, {}]) {
    assert.throws(() => orders.cancel(placed.id, reason), TypeError);
  }
  assert.equal(orders.get(placed.id).status, 'placed');
  assert.equal(orders.get(placed.id).reason, undefined);
  assert.equal(inventory.quantity('apple'), 8);
  assert.equal(events.length, 0);
});

test('reports exclude cancelled orders', () => {
  const { orders, reports } = setup();
  const keep = orders.place([{ sku: 'apple', qty: 2 }]);
  const drop = orders.place([{ sku: 'apple', qty: 3 }, { sku: 'pear', qty: 1 }]);
  assert.equal(reports.revenue(), 4 + 9);
  assert.equal(reports.unitsSold('apple'), 5);
  orders.cancel(drop.id, 'gone');
  assert.equal(reports.revenue(), 4);
  assert.equal(reports.unitsSold('apple'), 2);
  assert.equal(reports.unitsSold('pear'), 0);
  assert.equal(orders.get(keep.id).status, 'placed');
});
