const test = require('node:test');
const assert = require('node:assert');
const { total } = require('../src/cart.js');

test('single item', () => {
  assert.strictEqual(total([{ price: 10, qty: 1 }]), 10);
});

test('empty cart', () => {
  assert.strictEqual(total([]), 0);
});

test('calculates quantities and rounds to cents', () => {
  assert.strictEqual(total([{ price: 19.90, qty: 3 }]), 59.70);
  assert.strictEqual(total([{ price: 0.1, qty: 1 }, { price: 0.2, qty: 1 }]), 0.30);
});

test('applies percentage discounts to the total', () => {
  assert.strictEqual(total([{ price: 100, qty: 1 }], 'OFF20'), 80);
});

test('never returns a negative total after a fixed discount', () => {
  assert.strictEqual(total([{ price: 5, qty: 1 }], 'TENOFF'), 0);
});

test('ignores unknown coupons', () => {
  assert.strictEqual(total([{ price: 10, qty: 1 }], 'UNKNOWN'), 10);
});
