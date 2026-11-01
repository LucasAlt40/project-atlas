'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const { createLimiter } = require('../src/limiter');

function clock(start = 0) {
  let t = start;
  return {
    now: () => t,
    set: (ms) => {
      t = ms;
    },
    advance: (ms) => {
      t += ms;
    },
  };
}

test('starts full', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.available(), 5);
});

test('rejects invalid capacity', () => {
  for (const capacity of [undefined, null, '5', 0, -1, 1.5, NaN, Infinity]) {
    assert.throws(() => createLimiter({ capacity, refillPerSecond: 1 }), RangeError, `capacity ${capacity}`);
  }
});

test('rejects invalid refillPerSecond', () => {
  for (const refillPerSecond of [undefined, null, '1', 0, -1, NaN, Infinity]) {
    assert.throws(() => createLimiter({ capacity: 5, refillPerSecond }), RangeError, `refillPerSecond ${refillPerSecond}`);
  }
});

test('throws RangeError when called without options', () => {
  assert.throws(() => createLimiter(), RangeError);
});

test('accepts fractional refillPerSecond', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 0.5, now: c.now });
  assert.strictEqual(limiter.tryRemove(5), true);
  c.advance(2000);
  assert.strictEqual(limiter.available(), 1);
});

test('tryRemove removes tokens when enough are available', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(2), true);
  assert.strictEqual(limiter.available(), 2);
});

test('tryRemove removes nothing when not enough are available', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.tryRemove(4), true);
  assert.strictEqual(limiter.tryRemove(2), false);
  assert.strictEqual(limiter.available(), 1);
});

test('tryRemove rejects invalid n', () => {
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock().now });
  for (const n of [0, -1, 1.5, '2', NaN, Infinity]) {
    assert.throws(() => limiter.tryRemove(n), RangeError, `n ${n}`);
  }
});

test('tryRemove throws when n exceeds capacity', () => {
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock().now });
  assert.throws(() => limiter.tryRemove(6), RangeError);
});

test('refills continuously and available rounds down', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.tryRemove(10), true);
  c.advance(2500);
  assert.strictEqual(limiter.available(), 2);
});

test('refill never exceeds capacity', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.tryRemove(1), true);
  c.advance(1000000);
  assert.strictEqual(limiter.available(), 5);
});

test('msUntil is 0 when tokens are already available', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.msUntil(5), 0);
});

test('msUntil returns whole milliseconds until n tokens are available', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.tryRemove(5), true);
  assert.strictEqual(limiter.msUntil(3), 3000);
  c.advance(1500);
  assert.strictEqual(limiter.msUntil(3), 1500);
});

test('msUntil rounds up to the next millisecond', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 3, now: c.now });
  assert.strictEqual(limiter.tryRemove(5), true);
  // one token at 3 tokens/second takes 333.33... ms
  assert.strictEqual(limiter.msUntil(1), 334);
});

test('msUntil rejects invalid n', () => {
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock().now });
  assert.throws(() => limiter.msUntil(0), RangeError);
  assert.throws(() => limiter.msUntil(1.5), RangeError);
  assert.throws(() => limiter.msUntil(6), RangeError);
});

test('reset makes the bucket full again', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.tryRemove(5), true);
  limiter.reset();
  assert.strictEqual(limiter.available(), 5);
});

test('a clock going backwards adds no tokens and loses none', () => {
  const c = clock(1000);
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.tryRemove(5), true);
  c.set(0);
  assert.strictEqual(limiter.available(), 5);
  c.set(1000);
  assert.strictEqual(limiter.available(), 6);
});

test('after a backwards clock, refill is measured from the earlier time', () => {
  const c = clock(1000);
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.tryRemove(10), true);
  c.set(500);
  assert.strictEqual(limiter.available(), 0);
  c.set(1500);
  assert.strictEqual(limiter.available(), 1);
});
