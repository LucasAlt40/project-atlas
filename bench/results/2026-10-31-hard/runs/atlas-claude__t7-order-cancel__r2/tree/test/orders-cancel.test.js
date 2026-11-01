const test = require('node:test');
const assert = require('node:assert');
const { createShop } = require('../src/shop.js');

const setup = () => createShop({ stock: { pen: 10, cup: 2 }, prices: { pen: 1.5, cup: 8 } });

test('cancel returns items to stock, marks the order and announces it', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const order = shop.orders.place([{ sku: 'pen', qty: 2 }, { sku: 'cup', qty: 1 }]);
  const cancelled = shop.orders.cancel(order.id, 'changed my mind');
  assert.strictEqual(shop.inventory.quantity('pen'), 10);
  assert.strictEqual(shop.inventory.quantity('cup'), 2);
  assert.strictEqual(cancelled.status, 'cancelled');
  assert.strictEqual(cancelled.reason, 'changed my mind');
  assert.strictEqual(shop.orders.get(order.id).status, 'cancelled');
  assert.deepStrictEqual(seen, [{ orderId: order.id, reason: 'changed my mind' }]);
});

test('cancel of an unknown order throws not found and changes nothing', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  assert.throws(() => shop.orders.cancel(99, 'oops'), (err) => err instanceof Error && /not found/.test(err.message));
  assert.deepStrictEqual(seen, []);
});

test('cancelling twice throws already cancelled and does not restock again', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const order = shop.orders.place([{ sku: 'pen', qty: 3 }]);
  shop.orders.cancel(order.id, 'first');
  assert.throws(() => shop.orders.cancel(order.id, 'second'), (err) => err instanceof Error && /already cancelled/.test(err.message));
  assert.strictEqual(shop.inventory.quantity('pen'), 10);
  assert.strictEqual(shop.orders.get(order.id).reason, 'first');
  assert.deepStrictEqual(seen, [{ orderId: order.id, reason: 'first' }]);
});

test('an empty, whitespace-only or non-string reason throws TypeError and changes nothing', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const order = shop.orders.place([{ sku: 'pen', qty: 2 }]);
  for (const reason of ['', '   ', undefined, null, 42]) {
    assert.throws(() => shop.orders.cancel(order.id, reason), TypeError);
  }
  assert.strictEqual(shop.inventory.quantity('pen'), 8);
  assert.strictEqual(shop.orders.get(order.id).status, 'placed');
  assert.deepStrictEqual(seen, []);
});

test('reason is checked before the order lookup', () => {
  const shop = setup();
  assert.throws(() => shop.orders.cancel(99, ''), TypeError);
});

test('reports leave out cancelled orders', () => {
  const shop = setup();
  shop.orders.place([{ sku: 'pen', qty: 2 }]);
  const second = shop.orders.place([{ sku: 'pen', qty: 1 }, { sku: 'cup', qty: 1 }]);
  shop.orders.cancel(second.id, 'duplicate');
  assert.strictEqual(shop.reports.revenue(), 3);
  assert.strictEqual(shop.reports.unitsSold('pen'), 2);
  assert.strictEqual(shop.reports.unitsSold('cup'), 0);
});
