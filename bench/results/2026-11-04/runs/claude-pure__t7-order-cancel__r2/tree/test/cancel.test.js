const test = require('node:test');
const assert = require('node:assert');
const { createShop } = require('../src/shop.js');

const setup = () => createShop({ stock: { pen: 10, cup: 2 }, prices: { pen: 1.5, cup: 8 } });

test('cancel restores stock, marks the order and announces', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const order = shop.orders.place([{ sku: 'pen', qty: 2 }, { sku: 'cup', qty: 1 }]);
  const cancelled = shop.orders.cancel(order.id, 'changed my mind');
  assert.strictEqual(cancelled.id, order.id);
  assert.strictEqual(cancelled.status, 'cancelled');
  assert.strictEqual(cancelled.reason, 'changed my mind');
  assert.strictEqual(shop.inventory.quantity('pen'), 10);
  assert.strictEqual(shop.inventory.quantity('cup'), 2);
  assert.strictEqual(shop.orders.get(order.id).status, 'cancelled');
  assert.deepStrictEqual(seen, [{ orderId: order.id, reason: 'changed my mind' }]);
});

test('cancelling a missing order throws not found', () => {
  const shop = setup();
  assert.throws(() => shop.orders.cancel(99, 'nope'), (err) => err instanceof Error && /not found/.test(err.message));
});

test('cancelling twice throws and changes nothing', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const order = shop.orders.place([{ sku: 'pen', qty: 2 }]);
  shop.orders.cancel(order.id, 'first');
  assert.throws(() => shop.orders.cancel(order.id, 'second'), (err) => err instanceof Error && /already cancelled/.test(err.message));
  assert.strictEqual(shop.inventory.quantity('pen'), 10);
  assert.strictEqual(shop.orders.get(order.id).reason, 'first');
  assert.strictEqual(seen.length, 1);
});

test('an empty or non-string reason throws TypeError and changes nothing', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const order = shop.orders.place([{ sku: 'pen', qty: 2 }]);
  for (const reason of ['', '   ', undefined, null, 42]) {
    assert.throws(() => shop.orders.cancel(order.id, reason), TypeError);
  }
  assert.strictEqual(shop.inventory.quantity('pen'), 8);
  assert.strictEqual(shop.orders.get(order.id).status, 'placed');
  assert.strictEqual(seen.length, 0);
});

test('reports leave out cancelled orders', () => {
  const shop = setup();
  const kept = shop.orders.place([{ sku: 'pen', qty: 2 }]);
  const dropped = shop.orders.place([{ sku: 'pen', qty: 1 }, { sku: 'cup', qty: 1 }]);
  assert.strictEqual(shop.reports.revenue(), 12.5);
  assert.strictEqual(shop.reports.unitsSold('pen'), 3);
  shop.orders.cancel(dropped.id, 'out of stock elsewhere');
  assert.strictEqual(shop.reports.revenue(), 3);
  assert.strictEqual(shop.reports.unitsSold('pen'), 2);
  assert.strictEqual(shop.reports.unitsSold('cup'), 0);
  assert.strictEqual(shop.orders.get(kept.id).status, 'placed');
});
