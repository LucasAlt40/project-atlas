const test = require('node:test');
const assert = require('node:assert');
const { total } = require('../src/cart.js');

test('single item', () => {
  assert.strictEqual(total([{ price: 10, qty: 1 }]), 10);
});

test('empty cart', () => {
  assert.strictEqual(total([]), 0);
});

test('regression: quantity multiplies the price', () => {
  assert.strictEqual(total([{ price: 19.9, qty: 3 }]), 59.7);
});

test('regression: fixed coupon never makes the total negative', () => {
  assert.strictEqual(total([{ price: 5, qty: 1 }], 'TENOFF'), 0);
});

test('regression: percent coupon subtracts the discount', () => {
  assert.strictEqual(total([{ price: 100, qty: 1 }], 'OFF20'), 80);
  assert.strictEqual(total([{ price: 19.9, qty: 3 }], 'OFF20'), 47.76);
  assert.strictEqual(total([{ price: 10, qty: 1 }], 'HALF'), 5);
});

test('regression: totals are rounded to cents', () => {
  assert.strictEqual(
    total([
      { price: 0.1, qty: 1 },
      { price: 0.2, qty: 1 },
    ]),
    0.3
  );
});

test('regression: unknown coupons are ignored', () => {
  assert.strictEqual(total([{ price: 5, qty: 1 }], 'NOPE'), 5);
  assert.strictEqual(total([{ price: 5, qty: 1 }], 'toString'), 5);
  assert.strictEqual(total([{ price: 5, qty: 1 }], '__proto__'), 5);
});
