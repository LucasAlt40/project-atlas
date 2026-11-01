const test = require('node:test');
const assert = require('node:assert');
const { createShop } = require('../src/shop.js');

const setup = () => createShop({ stock: { pen: 10, cup: 2 }, prices: { pen: 1.5, cup: 8 } });

test('legacy: place and reports', () => {
  const shop = setup();
  shop.orders.place([{ sku: 'pen', qty: 2 }]);
  assert.strictEqual(shop.reports.revenue(), 3);
  assert.strictEqual(shop.inventory.quantity('pen'), 8);
});
test('cancel returns stock', () => {
  const shop = setup();
  const o = shop.orders.place([{ sku: 'pen', qty: 2 }, { sku: 'cup', qty: 1 }]);
  shop.orders.cancel(o.id, 'customer asked');
  assert.strictEqual(shop.inventory.quantity('pen'), 10);
  assert.strictEqual(shop.inventory.quantity('cup'), 2);
});
test('cancel marks status and reason, returns the order', () => {
  const shop = setup();
  const o = shop.orders.place([{ sku: 'pen', qty: 1 }]);
  const c = shop.orders.cancel(o.id, 'dup');
  assert.strictEqual(c.status, 'cancelled');
  assert.strictEqual(c.reason, 'dup');
  assert.strictEqual(shop.orders.get(o.id).status, 'cancelled');
});
test('cancel emits exactly one event', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const o = shop.orders.place([{ sku: 'pen', qty: 1 }]);
  shop.orders.cancel(o.id, 'dup');
  assert.deepStrictEqual(seen, [{ orderId: o.id, reason: 'dup' }]);
});
test('unknown order', () => {
  const shop = setup();
  assert.throws(() => shop.orders.cancel(999, 'x'), (e) => e instanceof Error && /not found/.test(e.message));
});
test('double cancel changes nothing', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.cancelled', (e) => seen.push(e));
  const o = shop.orders.place([{ sku: 'pen', qty: 2 }]);
  shop.orders.cancel(o.id, 'a');
  assert.throws(() => shop.orders.cancel(o.id, 'b'), (e) => /already cancelled/.test(e.message));
  assert.strictEqual(shop.inventory.quantity('pen'), 10);
  assert.strictEqual(seen.length, 1);
  assert.strictEqual(shop.orders.get(o.id).reason, 'a');
});
test('bad reason is a TypeError and changes nothing', () => {
  const shop = setup();
  const o = shop.orders.place([{ sku: 'pen', qty: 2 }]);
  for (const bad of ['', undefined, 42, '   '.trim()]) assert.throws(() => shop.orders.cancel(o.id, bad), TypeError);
  assert.strictEqual(shop.orders.get(o.id).status, 'placed');
  assert.strictEqual(shop.inventory.quantity('pen'), 8);
});
test('reports skip cancelled orders', () => {
  const shop = setup();
  const a = shop.orders.place([{ sku: 'pen', qty: 2 }]);
  shop.orders.place([{ sku: 'pen', qty: 1 }, { sku: 'cup', qty: 1 }]);
  shop.orders.cancel(a.id, 'x');
  assert.strictEqual(shop.reports.revenue(), 9.5);
  assert.strictEqual(shop.reports.unitsSold('pen'), 1);
});
test('stock returned can be ordered again', () => {
  const shop = setup();
  const a = shop.orders.place([{ sku: 'cup', qty: 2 }]);
  assert.throws(() => shop.orders.place([{ sku: 'cup', qty: 1 }]));
  shop.orders.cancel(a.id, 'x');
  assert.doesNotThrow(() => shop.orders.place([{ sku: 'cup', qty: 2 }]));
});
test('placed event unchanged', () => {
  const shop = setup();
  const seen = [];
  shop.bus.on('order.placed', (e) => seen.push(e));
  const o = shop.orders.place([{ sku: 'cup', qty: 1 }]);
  assert.deepStrictEqual(seen, [{ orderId: o.id, total: 8 }]);
});
