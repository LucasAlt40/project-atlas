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

test('restore puts stock back', () => {
  const shop = setup();
  shop.inventory.take('pen', 4);
  shop.inventory.restore('pen', 3);
  assert.strictEqual(shop.inventory.quantity('pen'), 9);
});

test('cancel restores stock, marks the order and announces once', () => {
  const shop = setup();
  const order = shop.orders.place([{ sku: 'pen', qty: 2 }, { sku: 'cup', qty: 1 }]);
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const cancelled = shop.orders.cancel(order.id, 'changed mind');
  assert.strictEqual(shop.inventory.quantity('pen'), 10);
  assert.strictEqual(shop.inventory.quantity('cup'), 2);
  assert.strictEqual(cancelled.status, 'cancelled');
  assert.strictEqual(cancelled.reason, 'changed mind');
  assert.strictEqual(shop.orders.get(order.id).status, 'cancelled');
  assert.deepStrictEqual(seen, [{ orderId: order.id, reason: 'changed mind' }]);
});

test('cancel of an unknown order throws not found and changes nothing', () => {
  const shop = setup();
  shop.orders.place([{ sku: 'pen', qty: 2 }]);
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  assert.throws(() => shop.orders.cancel(99, 'oops'), /not found/);
  assert.strictEqual(shop.inventory.quantity('pen'), 8);
  assert.deepStrictEqual(seen, []);
});

test('cancelling twice throws already cancelled and changes nothing', () => {
  const shop = setup();
  const order = shop.orders.place([{ sku: 'pen', qty: 2 }]);
  shop.orders.cancel(order.id, 'first');
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  assert.throws(() => shop.orders.cancel(order.id, 'second'), /already cancelled/);
  assert.strictEqual(shop.inventory.quantity('pen'), 10);
  assert.strictEqual(shop.orders.get(order.id).reason, 'first');
  assert.deepStrictEqual(seen, []);
});

test('cancel rejects a missing or blank reason with TypeError and changes nothing', () => {
  const shop = setup();
  const order = shop.orders.place([{ sku: 'pen', qty: 2 }]);
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  for (const reason of [undefined, null, 42, '', '   ']) {
    assert.throws(() => shop.orders.cancel(order.id, reason), TypeError);
  }
  assert.strictEqual(shop.inventory.quantity('pen'), 8);
  assert.strictEqual(shop.orders.get(order.id).status, 'placed');
  assert.deepStrictEqual(seen, []);
});

test('reports skip cancelled orders', () => {
  const shop = setup();
  const first = shop.orders.place([{ sku: 'pen', qty: 2 }]);
  shop.orders.place([{ sku: 'pen', qty: 1 }, { sku: 'cup', qty: 1 }]);
  shop.orders.cancel(first.id, 'duplicate');
  assert.strictEqual(shop.reports.revenue(), 9.5);
  assert.strictEqual(shop.reports.unitsSold('pen'), 1);
  assert.strictEqual(shop.orders.all().length, 2);
});
