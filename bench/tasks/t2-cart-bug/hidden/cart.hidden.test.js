const test = require('node:test');
const assert = require('node:assert');
const { total } = require('../src/cart.js');

test('qty is used', () => assert.strictEqual(total([{ price: 19.9, qty: 3 }]), 59.7));
test('several items', () => assert.strictEqual(total([{ price: 2.5, qty: 2 }, { price: 1, qty: 3 }]), 8));
test('rounded to cents', () => assert.strictEqual(total([{ price: 0.1, qty: 3 }]), 0.3));
test('fixed coupon', () => assert.strictEqual(total([{ price: 30, qty: 1 }], 'TENOFF'), 20));
test('fixed coupon never negative', () => assert.strictEqual(total([{ price: 5, qty: 1 }], 'TENOFF'), 0));
test('percent coupon 20', () => assert.strictEqual(total([{ price: 100, qty: 1 }], 'OFF20'), 80));
test('percent coupon 50 with qty', () => assert.strictEqual(total([{ price: 19.9, qty: 3 }], 'HALF'), 29.85));
test('unknown coupon ignored', () => assert.strictEqual(total([{ price: 10, qty: 2 }], 'NOPE'), 20));
test('empty cart', () => assert.strictEqual(total([]), 0));
test('empty cart with coupon', () => assert.strictEqual(total([], 'TENOFF'), 0));
test('original tests still hold', () => assert.strictEqual(total([{ price: 10, qty: 1 }]), 10));
