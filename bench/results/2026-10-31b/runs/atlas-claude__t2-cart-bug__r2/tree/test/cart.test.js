const test = require('node:test');
const assert = require('node:assert');
const { total } = require('../src/cart.js');

test('single item', () => {
  assert.strictEqual(total([{ price: 10, qty: 1 }]), 10);
});

test('empty cart', () => {
  assert.strictEqual(total([]), 0);
});

test('multiplies price by quantity', () => {
  assert.strictEqual(total([{ price: 19.9, qty: 3 }]), 59.7);
  assert.strictEqual(total([{ price: 10, qty: 2 }, { price: 5, qty: 1 }]), 25);
});

test('fixed coupon never makes the total negative', () => {
  assert.strictEqual(total([{ price: 5, qty: 1 }], 'TENOFF'), 0);
  assert.strictEqual(total([{ price: 30, qty: 1 }], 'TENOFF'), 20);
});

test('percent coupon returns the price after discount', () => {
  assert.strictEqual(total([{ price: 100, qty: 1 }], 'OFF20'), 80);
  assert.strictEqual(total([{ price: 100, qty: 1 }], 'HALF'), 50);
});

test('totals are rounded to cents', () => {
  assert.strictEqual(total([{ price: 0.1, qty: 1 }, { price: 0.2, qty: 1 }]), 0.3);
});

test('unknown coupons are ignored', () => {
  assert.strictEqual(total([{ price: 10, qty: 1 }], 'NOPE'), 10);
  assert.strictEqual(total([{ price: 10, qty: 1 }], 'constructor'), 10);
  assert.strictEqual(total([{ price: 10, qty: 1 }], '__proto__'), 10);
});
