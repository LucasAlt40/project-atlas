const test = require('node:test');
const assert = require('node:assert');
const { total } = require('../src/cart.js');

test('single item', () => {
  assert.strictEqual(total([{ price: 10, qty: 1 }]), 10);
});

test('empty cart', () => {
  assert.strictEqual(total([]), 0);
});

test('regression: quantity, coupons, clamping and cent rounding', () => {
  // Quantity multiplies the price.
  assert.strictEqual(total([{ price: 19.9, qty: 3 }]), 59.7);
  // Percent coupon takes the percentage off the subtotal.
  assert.strictEqual(total([{ price: 100, qty: 1 }], 'OFF20'), 80);
  // Fixed coupon never drives the total below zero.
  assert.strictEqual(total([{ price: 5, qty: 1 }], 'TENOFF'), 0);
  // Unknown coupons, including inherited property names, are ignored.
  assert.strictEqual(total([{ price: 10, qty: 1 }], 'NOPE'), 10);
  assert.strictEqual(total([{ price: 10, qty: 1 }], 'constructor'), 10);
  // Results are rounded to cents, never long decimals.
  assert.strictEqual(total([{ price: 0.1, qty: 3 }]), 0.3);
  assert.strictEqual(total([{ price: 0.1, qty: 1 }, { price: 0.2, qty: 1 }]), 0.3);
});
