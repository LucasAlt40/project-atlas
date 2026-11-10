const test = require('node:test');
const assert = require('node:assert');
const { createShop } = require('../src/shop.js');

const setup = () => createShop({ stock: { pen: 10, cup: 2 }, prices: { pen: 1.5, cup: 8 } });

test('cancel restocks items, marks the order and announces it', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const order = shop.orders.place([{ sku: 'pen', qty: 2 }, { sku: 'cup', qty: 1 }]);
  const cancelled = shop.orders.cancel(order.id, 'changed my mind');
  assert.strictEqual(cancelled.status, 'cancelled');
  assert.strictEqual(cancelled.reason, 'changed my mind');
  assert.strictEqual(shop.orders.get(order.id).status, 'cancelled');
  assert.strictEqual(shop.inventory.quantity('pen'), 10);
  assert.strictEqual(shop.inventory.quantity('cup'), 2);
  assert.deepStrictEqual(seen, [{ orderId: order.id, reason: 'changed my mind' }]);
});

test('cancelling a missing order throws not found', () => {
  const shop = setup();
  assert.throws(() => shop.orders.cancel(99, 'nope'), (err) => err instanceof Error && err.message.includes('not found'));
});

test('cancelling twice throws and changes nothing', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const order = shop.orders.place([{ sku: 'pen', qty: 2 }]);
  shop.orders.cancel(order.id, 'first');
  assert.throws(() => shop.orders.cancel(order.id, 'second'), (err) => err instanceof Error && err.message.includes('already cancelled'));
  assert.strictEqual(shop.inventory.quantity('pen'), 10);
  assert.strictEqual(shop.orders.get(order.id).reason, 'first');
  assert.strictEqual(seen.length, 1);
});

test('an empty or non-string reason throws a TypeError and changes nothing', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const order = shop.orders.place([{ sku: 'pen', qty: 2 }]);
  for (const reason of ['', '   ', undefined, null, 42]) {
    assert.throws(() => shop.orders.cancel(order.id, reason), TypeError);
  }
  assert.strictEqual(shop.orders.get(order.id).status, 'placed');
  assert.strictEqual(shop.inventory.quantity('pen'), 8);
  assert.strictEqual(seen.length, 0);
});

test('reports leave out cancelled orders', () => {
  const shop = setup();
  const first = shop.orders.place([{ sku: 'pen', qty: 2 }]);
  shop.orders.place([{ sku: 'pen', qty: 1 }, { sku: 'cup', qty: 1 }]);
  shop.orders.cancel(first.id, 'dup');
  assert.strictEqual(shop.reports.revenue(), 9.5);
  assert.strictEqual(shop.reports.unitsSold('pen'), 1);
  assert.strictEqual(shop.reports.unitsSold('cup'), 1);
});

test('stock can be placed again after a cancel', () => {
  const shop = setup();
  const order = shop.orders.place([{ sku: 'cup', qty: 2 }]);
  assert.strictEqual(shop.inventory.quantity('cup'), 0);
  shop.orders.cancel(order.id, 'out of stock elsewhere');
  shop.orders.place([{ sku: 'cup', qty: 2 }]);
  assert.strictEqual(shop.inventory.quantity('cup'), 0);
});
