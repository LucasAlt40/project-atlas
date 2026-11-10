const test = require('node:test');
const assert = require('node:assert');
const { total } = require('../src/cart.js');

test('single item', () => {
  assert.strictEqual(total([{ price: 10, qty: 1 }]), 10);
});

test('empty cart', () => {
  assert.strictEqual(total([]), 0);
});

test('quantity multiplies the price', () => {
  assert.strictEqual(total([{ price: 19.9, qty: 3 }]), 59.7);
});

test('fixed coupon never makes the total negative', () => {
  assert.strictEqual(total([{ price: 5, qty: 1 }], 'TENOFF'), 0);
});

test('percent coupon returns the discounted total, not the discount', () => {
  assert.strictEqual(total([{ price: 100, qty: 1 }], 'OFF20'), 80);
});

test('total is rounded to cents', () => {
  assert.strictEqual(
    total([
      { price: 0.1, qty: 1 },
      { price: 0.2, qty: 1 },
    ]),
    0.3,
  );
});

test('unknown coupon is ignored', () => {
  assert.strictEqual(total([{ price: 10, qty: 1 }], 'NOPE'), 10);
});

test('inherited object property is not treated as a coupon', () => {
  assert.strictEqual(total([{ price: 10, qty: 1 }], 'toString'), 10);
});
