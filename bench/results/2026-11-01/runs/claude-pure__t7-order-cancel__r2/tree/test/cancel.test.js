const test = require('node:test');
const assert = require('node:assert');
const { createShop } = require('../src/shop.js');

const setup = () => createShop({ stock: { pen: 10, cup: 2 }, prices: { pen: 1.5, cup: 8 } });

test('cancel returns stock and marks the order cancelled with its reason', () => {
  const shop = setup();
  const placed = shop.orders.place([{ sku: 'pen', qty: 2 }, { sku: 'cup', qty: 1 }]);
  const cancelled = shop.orders.cancel(placed.id, 'changed my mind');
  assert.strictEqual(shop.inventory.quantity('pen'), 10);
  assert.strictEqual(shop.inventory.quantity('cup'), 2);
  assert.strictEqual(cancelled.status, 'cancelled');
  assert.strictEqual(cancelled.reason, 'changed my mind');
  assert.strictEqual(shop.orders.get(placed.id).status, 'cancelled');
  assert.strictEqual(shop.orders.get(placed.id).reason, 'changed my mind');
});

test('cancel emits one order.cancelled event', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const placed = shop.orders.place([{ sku: 'pen', qty: 1 }]);
  shop.orders.cancel(placed.id, 'duplicate');
  assert.deepStrictEqual(seen, [{ orderId: placed.id, reason: 'duplicate' }]);
});

test('cancelling an unknown order throws not found', () => {
  const shop = setup();
  assert.throws(() => shop.orders.cancel(99, 'nope'), (err) => err instanceof Error && /not found/.test(err.message));
});

test('cancelling twice throws and changes nothing', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const placed = shop.orders.place([{ sku: 'pen', qty: 3 }]);
  shop.orders.cancel(placed.id, 'first');
  assert.throws(() => shop.orders.cancel(placed.id, 'second'), (err) => err instanceof Error && /already cancelled/.test(err.message));
  assert.strictEqual(shop.inventory.quantity('pen'), 10);
  assert.strictEqual(shop.orders.get(placed.id).reason, 'first');
  assert.strictEqual(seen.length, 1);
});

test('an empty or non-string reason throws TypeError and changes nothing', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const placed = shop.orders.place([{ sku: 'pen', qty: 3 }]);
  for (const reason of ['', '   ', undefined, null, 42]) {
    assert.throws(() => shop.orders.cancel(placed.id, reason), TypeError);
  }
  assert.strictEqual(shop.orders.get(placed.id).status, 'placed');
  assert.strictEqual(shop.inventory.quantity('pen'), 7);
  assert.deepStrictEqual(seen, []);
});

test('reports leave out cancelled orders', () => {
  const shop = setup();
  shop.orders.place([{ sku: 'pen', qty: 2 }]);
  const cancelMe = shop.orders.place([{ sku: 'pen', qty: 1 }, { sku: 'cup', qty: 1 }]);
  shop.orders.cancel(cancelMe.id, 'no');
  assert.strictEqual(shop.reports.revenue(), 3);
  assert.strictEqual(shop.reports.unitsSold('pen'), 2);
  assert.strictEqual(shop.reports.unitsSold('cup'), 0);
});
