const test = require('node:test');
const assert = require('node:assert');
const { createShop } = require('../src/shop.js');

const setup = () => createShop({ stock: { pen: 10, cup: 2 }, prices: { pen: 1.5, cup: 8 } });

test('cancel restocks items, marks the order and announces it', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const placed = shop.orders.place([{ sku: 'pen', qty: 2 }, { sku: 'cup', qty: 1 }]);
  const cancelled = shop.orders.cancel(placed.id, 'customer asked');
  assert.strictEqual(cancelled.status, 'cancelled');
  assert.strictEqual(cancelled.reason, 'customer asked');
  assert.strictEqual(cancelled.id, placed.id);
  assert.strictEqual(shop.inventory.quantity('pen'), 10);
  assert.strictEqual(shop.inventory.quantity('cup'), 2);
  assert.deepStrictEqual(seen, [{ orderId: placed.id, reason: 'customer asked' }]);
  assert.strictEqual(shop.orders.get(placed.id).status, 'cancelled');
});

test('cancel of a missing order throws not found and changes nothing', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  assert.throws(() => shop.orders.cancel(99, 'nope'), (err) => err instanceof Error && /not found/.test(err.message));
  assert.deepStrictEqual(seen, []);
  assert.strictEqual(shop.inventory.quantity('pen'), 10);
});

test('cancelling twice throws already cancelled and changes nothing', () => {
  const shop = setup();
  const seen = [];
  const placed = shop.orders.place([{ sku: 'pen', qty: 3 }]);
  shop.orders.cancel(placed.id, 'first');
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  assert.throws(() => shop.orders.cancel(placed.id, 'second'), (err) => err instanceof Error && /already cancelled/.test(err.message));
  assert.deepStrictEqual(seen, []);
  assert.strictEqual(shop.inventory.quantity('pen'), 10);
  assert.strictEqual(shop.orders.get(placed.id).reason, 'first');
});

test('reason must be a non-empty string', () => {
  const shop = setup();
  const seen = [];
  const placed = shop.orders.place([{ sku: 'pen', qty: 3 }]);
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  for (const bad of ['', '   ', undefined, null, 42]) {
    assert.throws(() => shop.orders.cancel(placed.id, bad), TypeError);
  }
  assert.deepStrictEqual(seen, []);
  assert.strictEqual(shop.inventory.quantity('pen'), 7);
  assert.strictEqual(shop.orders.get(placed.id).status, 'placed');
});

test('reports leave out cancelled orders', () => {
  const shop = setup();
  const kept = shop.orders.place([{ sku: 'pen', qty: 2 }]);
  const dropped = shop.orders.place([{ sku: 'pen', qty: 1 }, { sku: 'cup', qty: 1 }]);
  shop.orders.cancel(dropped.id, 'changed mind');
  assert.strictEqual(shop.reports.revenue(), kept.total);
  assert.strictEqual(shop.reports.unitsSold('pen'), 2);
  assert.strictEqual(shop.reports.unitsSold('cup'), 0);
  assert.strictEqual(shop.orders.all().length, 2);
});

test('restock adds to stock for a sku', () => {
  const shop = setup();
  shop.inventory.restock('pen', 4);
  shop.inventory.restock('mug', 1);
  assert.strictEqual(shop.inventory.quantity('pen'), 14);
  assert.strictEqual(shop.inventory.quantity('mug'), 1);
});
