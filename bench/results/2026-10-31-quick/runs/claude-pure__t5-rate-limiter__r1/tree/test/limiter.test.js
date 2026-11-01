'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const { createLimiter } = require('../src/limiter');

function clock(start = 0) {
  const c = { t: start, now: () => c.t, advance: (ms) => { c.t += ms; } };
  return c;
}

test('exports createLimiter', () => {
  assert.deepStrictEqual(Object.keys(require('../src/limiter')), ['createLimiter']);
  assert.strictEqual(typeof createLimiter, 'function');
});

test('bucket starts full', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.available(), 5);
});

test('tryRemove removes tokens when available and fails when not', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.tryRemove(2), true);
  assert.strictEqual(limiter.available(), 1);
  assert.strictEqual(limiter.tryRemove(2), false);
  assert.strictEqual(limiter.available(), 1, 'failed request removes nothing');
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), false);
});

test('tryRemove defaults to one token', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: c.now });
  limiter.tryRemove();
  assert.strictEqual(limiter.available(), 1);
});

test('tokens refill continuously and never exceed capacity', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 2, now: c.now });
  assert.strictEqual(limiter.tryRemove(4), true);
  c.advance(500);
  assert.strictEqual(limiter.available(), 1);
  c.advance(10_000);
  assert.strictEqual(limiter.available(), 4, 'capped at capacity');
});

test('fractional refill rate accumulates and floors for available()', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 0.5, now: c.now });
  limiter.tryRemove(3);
  c.advance(1000);
  assert.strictEqual(limiter.available(), 0);
  c.advance(1000);
  assert.strictEqual(limiter.available(), 1);
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), false);
});

test('msUntil is 0 when tokens are already available', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.msUntil(), 0);
  assert.strictEqual(limiter.msUntil(3), 0);
});

test('msUntil rounds up to whole milliseconds', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 3, now: c.now });
  limiter.tryRemove(5);
  // one token takes 1000/3 ms, which rounds up to 334
  assert.strictEqual(limiter.msUntil(), 334);
  assert.strictEqual(limiter.msUntil(2), 667);
});

test('msUntil accounts for partially refilled tokens', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: c.now });
  limiter.tryRemove(2);
  c.advance(400);
  assert.strictEqual(limiter.msUntil(), 600);
});

test('waiting msUntil ms makes the request succeed', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 3, now: c.now });
  limiter.tryRemove(5);
  const wait = limiter.msUntil(2);
  c.advance(wait);
  assert.strictEqual(limiter.tryRemove(2), true);
});

test('reset makes the bucket full again', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 0.1, now: c.now });
  limiter.tryRemove(3);
  assert.strictEqual(limiter.available(), 0);
  limiter.reset();
  assert.strictEqual(limiter.available(), 3);
  assert.strictEqual(limiter.tryRemove(3), true);
});

test('a clock going backwards adds and loses nothing', () => {
  const c = clock(10_000);
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 1, now: c.now });
  limiter.tryRemove(5);
  c.t = 9_000;
  assert.strictEqual(limiter.available(), 5, 'no tokens added on backwards jump');
  c.advance(2000);
  assert.strictEqual(limiter.available(), 7, 'refill counts from the earlier reference');
});

test('a backwards clock resets the reference so later refill is measured from it', () => {
  const c = clock(5_000);
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 1, now: c.now });
  limiter.tryRemove(10);
  c.t = 1_000;
  assert.strictEqual(limiter.available(), 0);
  c.t = 3_000;
  assert.strictEqual(limiter.available(), 2);
});

test('invalid capacity throws RangeError', () => {
  for (const capacity of [undefined, 0, -1, 1.5, NaN, Infinity, '5', null]) {
    assert.throws(() => createLimiter({ capacity, refillPerSecond: 1 }), RangeError, `capacity ${capacity}`);
  }
});

test('invalid refillPerSecond throws RangeError', () => {
  for (const refillPerSecond of [undefined, 0, -1, NaN, Infinity, '1', null]) {
    assert.throws(() => createLimiter({ capacity: 1, refillPerSecond }), RangeError, `refill ${refillPerSecond}`);
  }
});

test('invalid token counts throw RangeError', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock().now });
  for (const n of [0, -1, 1.5, NaN, Infinity, '1', null]) {
    assert.throws(() => limiter.tryRemove(n), RangeError, `tryRemove(${n})`);
    assert.throws(() => limiter.msUntil(n), RangeError, `msUntil(${n})`);
  }
});

test('token count above capacity throws RangeError', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock().now });
  assert.throws(() => limiter.tryRemove(4), RangeError);
  assert.throws(() => limiter.msUntil(4), RangeError);
});
