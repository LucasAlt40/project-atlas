const test = require('node:test');
const assert = require('node:assert');
const { createShop } = require('../src/shop.js');

const setup = () => createShop({ stock: { pen: 10, cup: 2 }, prices: { pen: 1.5, cup: 8 } });

test('place takes stock and announces', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.placed', (e) => seen.push(e));
  const order = shop.orders.place([{ sku: 'pen', qty: 2 }, { sku: 'cup', qty: 1 }]);
  assert.strictEqual(order.total, 11);
  assert.strictEqual(shop.inventory.quantity('pen'), 8);
  assert.deepStrictEqual(seen, [{ orderId: order.id, total: 11 }]);
});

test('not enough stock takes nothing', () => {
  const shop = setup();
  assert.throws(() => shop.orders.place([{ sku: 'pen', qty: 1 }, { sku: 'cup', qty: 5 }]));
  assert.strictEqual(shop.inventory.quantity('pen'), 10);
});

test('reports', () => {
  const shop = setup();
  shop.orders.place([{ sku: 'pen', qty: 2 }]);
  shop.orders.place([{ sku: 'pen', qty: 1 }, { sku: 'cup', qty: 1 }]);
  assert.strictEqual(shop.reports.revenue(), 12.5);
  assert.strictEqual(shop.reports.unitsSold('pen'), 3);
});

test('restock adds to stock', () => {
  const shop = setup();
  shop.inventory.restock('pen', 4);
  shop.inventory.restock('lamp', 1);
  assert.strictEqual(shop.inventory.quantity('pen'), 14);
  assert.strictEqual(shop.inventory.quantity('lamp'), 1);
});

test('cancel restocks items, marks the order and announces', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const placed = shop.orders.place([{ sku: 'pen', qty: 2 }, { sku: 'cup', qty: 1 }]);
  const cancelled = shop.orders.cancel(placed.id, 'changed mind');
  assert.strictEqual(cancelled.id, placed.id);
  assert.strictEqual(cancelled.status, 'cancelled');
  assert.strictEqual(cancelled.reason, 'changed mind');
  assert.strictEqual(shop.inventory.quantity('pen'), 10);
  assert.strictEqual(shop.inventory.quantity('cup'), 2);
  assert.deepStrictEqual(seen, [{ orderId: placed.id, reason: 'changed mind' }]);
  assert.strictEqual(shop.orders.get(placed.id).status, 'cancelled');
});

test('cancel of a missing order throws not found', () => {
  const shop = setup();
  assert.throws(() => shop.orders.cancel(99, 'nope'), (err) => err instanceof Error && /not found/.test(err.message));
});

test('cancelling twice throws and changes nothing', () => {
  const shop = setup();
  const seen = [];
  const placed = shop.orders.place([{ sku: 'pen', qty: 2 }]);
  shop.orders.cancel(placed.id, 'first');
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  assert.throws(() => shop.orders.cancel(placed.id, 'second'), (err) => err instanceof Error && /already cancelled/.test(err.message));
  assert.strictEqual(shop.inventory.quantity('pen'), 10);
  assert.strictEqual(shop.orders.get(placed.id).reason, 'first');
  assert.deepStrictEqual(seen, []);
});

test('cancel with an empty or non-string reason throws TypeError and changes nothing', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const placed = shop.orders.place([{ sku: 'pen', qty: 2 }]);
  for (const reason of ['', '   ', undefined, null, 42]) {
    assert.throws(() => shop.orders.cancel(placed.id, reason), TypeError);
  }
  assert.strictEqual(shop.inventory.quantity('pen'), 8);
  assert.strictEqual(shop.orders.get(placed.id).status, 'placed');
  assert.deepStrictEqual(seen, []);
});

test('reason check runs before the order lookup', () => {
  const shop = setup();
  assert.throws(() => shop.orders.cancel(99, ''), TypeError);
});

test('cancelled orders are left out of reports', () => {
  const shop = setup();
  const first = shop.orders.place([{ sku: 'pen', qty: 2 }]);
  shop.orders.place([{ sku: 'pen', qty: 1 }, { sku: 'cup', qty: 1 }]);
  shop.orders.cancel(first.id, 'duplicate');
  assert.strictEqual(shop.reports.revenue(), 9.5);
  assert.strictEqual(shop.reports.unitsSold('pen'), 1);
  assert.strictEqual(shop.orders.all().length, 2);
});
