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

test('cancel restocks, marks cancelled with reason, and announces once', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const order = shop.orders.place([{ sku: 'pen', qty: 2 }, { sku: 'cup', qty: 1 }]);
  const cancelled = shop.orders.cancel(order.id, 'changed mind');
  assert.strictEqual(cancelled.status, 'cancelled');
  assert.strictEqual(cancelled.reason, 'changed mind');
  assert.strictEqual(shop.orders.get(order.id).status, 'cancelled');
  assert.strictEqual(shop.inventory.quantity('pen'), 10);
  assert.strictEqual(shop.inventory.quantity('cup'), 2);
  assert.deepStrictEqual(seen, [{ orderId: order.id, reason: 'changed mind' }]);
});

test('cancel an unknown order throws not found', () => {
  const shop = setup();
  assert.throws(() => shop.orders.cancel(99, 'oops'), (err) => err.message.includes('not found'));
});

test('cancel an already cancelled order changes nothing', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const order = shop.orders.place([{ sku: 'pen', qty: 2 }]);
  shop.orders.cancel(order.id, 'first');
  assert.throws(() => shop.orders.cancel(order.id, 'second'), (err) => err.message.includes('already cancelled'));
  assert.strictEqual(shop.inventory.quantity('pen'), 10);
  assert.strictEqual(shop.orders.get(order.id).reason, 'first');
  assert.strictEqual(seen.length, 1);
});

test('cancel with an empty or non-string reason throws TypeError and changes nothing', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const order = shop.orders.place([{ sku: 'pen', qty: 2 }]);
  for (const reason of ['', '   ', undefined, 42]) {
    assert.throws(() => shop.orders.cancel(order.id, reason), (err) => err instanceof TypeError && err.message.includes('non-empty string'));
  }
  assert.strictEqual(shop.orders.get(order.id).status, 'placed');
  assert.strictEqual(shop.inventory.quantity('pen'), 8);
  assert.deepStrictEqual(seen, []);
});

test('reports exclude cancelled orders', () => {
  const shop = setup();
  const kept = shop.orders.place([{ sku: 'pen', qty: 2 }]);
  const dropped = shop.orders.place([{ sku: 'pen', qty: 1 }, { sku: 'cup', qty: 1 }]);
  shop.orders.cancel(dropped.id, 'out of stock');
  assert.strictEqual(shop.reports.revenue(), kept.total);
  assert.strictEqual(shop.reports.unitsSold('pen'), 2);
  assert.strictEqual(shop.reports.unitsSold('cup'), 0);
});
