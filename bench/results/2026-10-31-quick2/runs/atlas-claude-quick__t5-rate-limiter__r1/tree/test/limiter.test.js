'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const { createLimiter } = require('../src/limiter');

function clock(start = 0) {
  const c = {
    t: start,
    now: () => c.t,
    advance(ms) {
      c.t += ms;
    },
  };
  return c;
}

test('starts full', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.available(), 5);
});

test('tryRemove takes tokens and returns true while available', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(2), true);
  assert.strictEqual(limiter.available(), 0);
});

test('tryRemove returns false and removes nothing when not enough tokens', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.tryRemove(2), true);
  assert.strictEqual(limiter.tryRemove(2), false);
  assert.strictEqual(limiter.available(), 1);
});

test('tokens refill continuously with elapsed time', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.tryRemove(2), true);
  c.advance(500);
  assert.strictEqual(limiter.available(), 0);
  c.advance(500);
  assert.strictEqual(limiter.available(), 1);
});

test('refill never exceeds capacity', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 10, now: c.now });
  assert.strictEqual(limiter.tryRemove(), true);
  c.advance(60000);
  assert.strictEqual(limiter.available(), 4);
});

test('fractional refill rate adds tokens correctly', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 0.5, now: c.now });
  assert.strictEqual(limiter.tryRemove(2), true);
  c.advance(1999);
  assert.strictEqual(limiter.available(), 0);
  c.advance(1);
  assert.strictEqual(limiter.available(), 1);
});

test('exact refill boundary is not one token short due to float error', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 0.1, now: c.now });
  assert.strictEqual(limiter.tryRemove(5), true);
  c.advance(10000);
  assert.strictEqual(limiter.available(), 1);
  assert.strictEqual(limiter.tryRemove(), true);
});

test('available rounds fractional tokens down', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.tryRemove(5), true);
  c.advance(2500);
  assert.strictEqual(limiter.available(), 2);
});

test('msUntil is 0 when tokens are already available', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.msUntil(), 0);
  assert.strictEqual(limiter.msUntil(5), 0);
});

test('msUntil rounds the wait up to whole milliseconds', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 3, now: c.now });
  assert.strictEqual(limiter.tryRemove(10), true);
  assert.strictEqual(limiter.msUntil(), 334);
  assert.strictEqual(limiter.msUntil(3), 1000);
  c.advance(100);
  assert.strictEqual(limiter.msUntil(), 234);
});

test('reset makes the bucket full again', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.tryRemove(3), true);
  limiter.reset();
  assert.strictEqual(limiter.available(), 3);
});

test('clock going backwards adds and loses no tokens, and the earlier time becomes the reference', () => {
  const c = clock(1000);
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.tryRemove(2), true);
  c.t = 0;
  assert.strictEqual(limiter.available(), 3);
  c.advance(500);
  assert.strictEqual(limiter.available(), 3);
  c.advance(500);
  assert.strictEqual(limiter.available(), 4);
});

test('createLimiter rejects invalid capacity', () => {
  const bad = [undefined, 0, -1, 1.5, NaN, Infinity, '3', null];
  for (const capacity of bad) {
    assert.throws(() => createLimiter({ capacity, refillPerSecond: 1 }), RangeError, `capacity ${String(capacity)}`);
  }
});

test('createLimiter rejects invalid refillPerSecond', () => {
  const bad = [undefined, 0, -1, NaN, Infinity, '1', null];
  for (const refillPerSecond of bad) {
    assert.throws(() => createLimiter({ capacity: 1, refillPerSecond }), RangeError, `refill ${String(refillPerSecond)}`);
  }
});

test('invalid n throws RangeError in tryRemove and msUntil', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: c.now });
  const bad = [0, -1, 1.5, NaN, '1', null, 4];
  for (const n of bad) {
    assert.throws(() => limiter.tryRemove(n), RangeError, `tryRemove ${String(n)}`);
    assert.throws(() => limiter.msUntil(n), RangeError, `msUntil ${String(n)}`);
  }
});

test('default clock is used when now is not given', () => {
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1 });
  assert.strictEqual(limiter.available(), 2);
  assert.strictEqual(limiter.tryRemove(2), true);
});
