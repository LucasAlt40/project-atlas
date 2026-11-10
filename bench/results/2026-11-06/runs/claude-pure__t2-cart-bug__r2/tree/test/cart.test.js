const test = require('node:test');
const assert = require('node:assert');
const { total } = require('../src/cart.js');

test('single item', () => {
  assert.strictEqual(total([{ price: 10, qty: 1 }]), 10);
});

test('empty cart', () => {
  assert.strictEqual(total([]), 0);
});

test('regression: quantity multiplies price and totals round to cents', () => {
  assert.strictEqual(total([{ price: 19.9, qty: 3 }]), 59.7);
  assert.strictEqual(total([{ price: 0.1, qty: 3 }, { price: 0.2, qty: 1 }]), 0.5);
});

test('regression: fixed coupon never makes the total negative', () => {
  assert.strictEqual(total([{ price: 5, qty: 1 }], 'TENOFF'), 0);
  assert.strictEqual(total([{ price: 30, qty: 1 }], 'TENOFF'), 20);
});

test('regression: percent coupon takes the discount off the total', () => {
  assert.strictEqual(total([{ price: 100, qty: 1 }], 'OFF20'), 80);
  assert.strictEqual(total([{ price: 100, qty: 1 }], 'HALF'), 50);
});

test('regression: unknown or inherited coupon codes are ignored', () => {
  assert.strictEqual(total([{ price: 10, qty: 1 }], 'NOPE'), 10);
  assert.strictEqual(total([{ price: 10, qty: 1 }], 'toString'), 10);
  assert.strictEqual(total([{ price: 10, qty: 1 }], '__proto__'), 10);
});

test('regression: no float noise in results', () => {
  const result = total([{ price: 0.1, qty: 1 }, { price: 0.2, qty: 1 }]);
  assert.strictEqual(result, 0.3);
  assert.strictEqual(total([{ price: 1.1, qty: 3 }], 'OFF20'), 2.64);
});
