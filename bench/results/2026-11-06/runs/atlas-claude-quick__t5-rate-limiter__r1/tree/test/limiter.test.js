'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const { createLimiter } = require('../src/limiter');

// Injected clock: time only moves when the test advances or sets it.
function fakeClock(start = 0) {
  let t = start;
  return {
    now: () => t,
    advance(ms) {
      t += ms;
    },
    set(ms) {
      t = ms;
    },
  };
}

test('module exports createLimiter', () => {
  assert.strictEqual(typeof createLimiter, 'function');
});

test('createLimiter rejects invalid capacity', () => {
  const valid = { refillPerSecond: 1 };
  for (const capacity of [undefined, null, 0, -1, 1.5, '3', NaN, Infinity]) {
    assert.throws(() => createLimiter({ ...valid, capacity }), RangeError, `capacity ${capacity}`);
  }
});

test('createLimiter rejects invalid refillPerSecond', () => {
  const valid = { capacity: 1 };
  for (const refillPerSecond of [undefined, null, 0, -1, '1', NaN, Infinity]) {
    assert.throws(() => createLimiter({ ...valid, refillPerSecond }), RangeError, `refill ${refillPerSecond}`);
  }
});

test('createLimiter rejects missing options', () => {
  assert.throws(() => createLimiter(), RangeError);
});

test('accepts a fractional refillPerSecond', () => {
  assert.doesNotThrow(() => createLimiter({ capacity: 1, refillPerSecond: 0.5 }));
});

test('bucket starts full and tryRemove drains it', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.available(), 3);
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(2), true);
  assert.strictEqual(limiter.available(), 0);
});

test('tryRemove fails without removing anything when tokens are short', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(2), true);
  assert.strictEqual(limiter.tryRemove(2), false);
  assert.strictEqual(limiter.available(), 1);
  assert.strictEqual(limiter.tryRemove(1), true);
});

test('tokens refill continuously with elapsed time', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 0.5, now: clock.now });
  limiter.tryRemove(4);
  clock.advance(1000);
  assert.strictEqual(limiter.available(), 0);
  clock.advance(1000);
  assert.strictEqual(limiter.available(), 1);
  clock.advance(1000);
  assert.strictEqual(limiter.available(), 1);
  clock.advance(1000);
  assert.strictEqual(limiter.available(), 2);
});

test('refill never exceeds capacity', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 10, now: clock.now });
  limiter.tryRemove(2);
  clock.advance(60_000);
  assert.strictEqual(limiter.available(), 2);
});

test('available rounds down fractional tokens', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 0.25, now: clock.now });
  limiter.tryRemove(5);
  clock.advance(3000);
  assert.strictEqual(limiter.available(), 0);
  clock.advance(1000);
  assert.strictEqual(limiter.available(), 1);
});

test('msUntil is 0 when tokens are already available', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.msUntil(), 0);
  assert.strictEqual(limiter.msUntil(3), 0);
});

test('msUntil returns the exact wait for whole-token deficits', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 0.5, now: clock.now });
  limiter.tryRemove(4);
  assert.strictEqual(limiter.msUntil(), 2000);
  assert.strictEqual(limiter.msUntil(2), 4000);
  clock.advance(1000);
  assert.strictEqual(limiter.msUntil(), 1000);
});

test('msUntil rounds up to whole milliseconds', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 3, now: clock.now });
  limiter.tryRemove(2);
  // One token at 3 per second takes 333.33 ms, so 334 ms.
  assert.strictEqual(limiter.msUntil(), 334);
  clock.advance(333);
  assert.strictEqual(limiter.tryRemove(), false);
  clock.advance(1);
  assert.strictEqual(limiter.tryRemove(), true);
});

test('msUntil agrees with tryRemove after the reported wait', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 2, now: clock.now });
  limiter.tryRemove(5);
  const wait = limiter.msUntil(3);
  assert.strictEqual(wait, 1500);
  clock.advance(wait - 1);
  assert.strictEqual(limiter.tryRemove(3), false);
  clock.advance(1);
  assert.strictEqual(limiter.tryRemove(3), true);
});

test('n must be a positive integer no greater than capacity', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  for (const n of [0, -1, 1.5, NaN, '1', Infinity]) {
    assert.throws(() => limiter.tryRemove(n), RangeError, `tryRemove ${n}`);
    assert.throws(() => limiter.msUntil(n), RangeError, `msUntil ${n}`);
  }
  assert.throws(() => limiter.tryRemove(4), RangeError);
  assert.throws(() => limiter.msUntil(4), RangeError);
});

test('reset makes the bucket full again', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 0.5, now: clock.now });
  limiter.tryRemove(3);
  limiter.reset();
  assert.strictEqual(limiter.available(), 3);
  assert.strictEqual(limiter.tryRemove(3), true);
});

test('clock going backwards adds no tokens and loses none', () => {
  const clock = fakeClock(1000);
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(3);
  assert.strictEqual(limiter.available(), 2);

  clock.set(500);
  assert.strictEqual(limiter.available(), 2);

  // The earlier time is the new reference, so refill resumes from 500 ms.
  clock.set(1500);
  assert.strictEqual(limiter.available(), 3);
});

test('defaults to Date.now when now is not given', () => {
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 1 });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), false);
});
