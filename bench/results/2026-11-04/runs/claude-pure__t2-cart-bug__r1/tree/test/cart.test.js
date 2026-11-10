const test = require('node:test');
const assert = require('node:assert');
const { total } = require('../src/cart.js');

test('single item', () => {
  assert.strictEqual(total([{ price: 10, qty: 1 }]), 10);
});

test('empty cart', () => {
  assert.strictEqual(total([]), 0);
});

test('regression: quantity multiplies price and totals are rounded to cents', () => {
  assert.strictEqual(total([{ price: 19.9, qty: 3 }]), 59.7);
  assert.strictEqual(total([{ price: 0.1, qty: 1 }, { price: 0.2, qty: 1 }]), 0.3);
});

test('regression: fixed coupon never makes the total negative', () => {
  assert.strictEqual(total([{ price: 5, qty: 1 }], 'TENOFF'), 0);
});

test('regression: percent coupon returns the discounted price', () => {
  assert.strictEqual(total([{ price: 100, qty: 1 }], 'OFF20'), 80);
});

test('regression: unknown and inherited coupon codes are ignored', () => {
  assert.strictEqual(total([{ price: 5, qty: 1 }], 'NOPE'), 5);
  assert.strictEqual(total([{ price: 5, qty: 1 }], 'toString'), 5);
});
