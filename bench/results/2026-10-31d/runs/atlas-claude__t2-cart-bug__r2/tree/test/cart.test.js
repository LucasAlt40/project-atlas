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

test('fixed coupon larger than the cart never gives a negative total', () => {
  assert.strictEqual(total([{ price: 5, qty: 1 }], 'TENOFF'), 0);
});

test('percent coupon leaves the discounted amount, not the discount', () => {
  assert.strictEqual(total([{ price: 100, qty: 1 }], 'OFF20'), 80);
});

test('totals are rounded to cents', () => {
  assert.strictEqual(total([{ price: 0.1, qty: 1 }, { price: 0.2, qty: 1 }]), 0.3);
});

test('unknown coupons are ignored', () => {
  assert.strictEqual(total([{ price: 10, qty: 1 }], 'NOPE'), 10);
  assert.strictEqual(total([{ price: 10, qty: 1 }], 'constructor'), 10);
});
