const { test } = require('node:test');
const assert = require('node:assert');
const { createLimiter } = require('../src/limiter');

function makeClock(start = 0) {
  const clock = {
    time: start,
    now: () => clock.time,
    advance: (ms) => {
      clock.time += ms;
    },
  };
  return clock;
}

test('rejects invalid capacity', () => {
  for (const capacity of [undefined, '2', 0, -1, 1.5, NaN, Infinity]) {
    assert.throws(
      () => createLimiter({ capacity, refillPerSecond: 1 }),
      RangeError,
      `capacity ${String(capacity)} should be rejected`,
    );
  }
});

test('rejects invalid refillPerSecond', () => {
  for (const refillPerSecond of [undefined, '1', 0, -1, NaN, Infinity]) {
    assert.throws(
      () => createLimiter({ capacity: 5, refillPerSecond }),
      RangeError,
      `refillPerSecond ${String(refillPerSecond)} should be rejected`,
    );
  }
});

test('accepts fractional refillPerSecond', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 0.5, now: clock.now });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.msUntil(), 2000);
});

test('starts full', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.available(), 3);
});

test('tryRemove removes tokens when available and fails when empty', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), false);
  assert.strictEqual(limiter.available(), 0);
});

test('tryRemove(n) is all-or-nothing', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.tryRemove(3), false);
  assert.strictEqual(limiter.available(), 2);
  assert.strictEqual(limiter.tryRemove(2), true);
  assert.strictEqual(limiter.available(), 0);
});

test('refills continuously with elapsed time', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(2);
  clock.advance(500);
  assert.strictEqual(limiter.available(), 0);
  assert.strictEqual(limiter.tryRemove(), false);
  clock.advance(500);
  assert.strictEqual(limiter.available(), 1);
  assert.strictEqual(limiter.tryRemove(), true);
});

test('fractional refill accumulates across calls', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 0.5, now: clock.now });
  limiter.tryRemove(3);
  clock.advance(1000);
  assert.strictEqual(limiter.available(), 0);
  clock.advance(1000);
  assert.strictEqual(limiter.available(), 1);
});

test('never refills above capacity', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 10, now: clock.now });
  limiter.tryRemove(1);
  clock.advance(60_000);
  assert.strictEqual(limiter.available(), 3);
  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.tryRemove(), false);
});

test('available rounds the token count down', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 2, now: clock.now });
  limiter.tryRemove(10);
  clock.advance(1700);
  assert.strictEqual(limiter.available(), 3);
});

test('n must be a positive integer', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  for (const n of [0, -1, 1.5, NaN, '1', null]) {
    assert.throws(() => limiter.tryRemove(n), RangeError, `tryRemove(${String(n)}) should throw`);
    assert.throws(() => limiter.msUntil(n), RangeError, `msUntil(${String(n)}) should throw`);
  }
});

test('n greater than capacity throws', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  assert.throws(() => limiter.tryRemove(6), RangeError);
  assert.throws(() => limiter.msUntil(6), RangeError);
  assert.strictEqual(limiter.available(), 5);
});

test('msUntil is 0 when tokens are already available', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.msUntil(), 0);
  assert.strictEqual(limiter.msUntil(4), 0);
});

test('msUntil returns the wait for the requested tokens', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 2, now: clock.now });
  limiter.tryRemove(4);
  assert.strictEqual(limiter.msUntil(), 500);
  assert.strictEqual(limiter.msUntil(3), 1500);
});

test('msUntil rounds up to whole milliseconds', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 3, now: clock.now });
  limiter.tryRemove(1);
  // One token takes 333.33... ms, so the answer is 334.
  assert.strictEqual(limiter.msUntil(), 334);
  clock.advance(334);
  assert.strictEqual(limiter.msUntil(), 0);
  assert.strictEqual(limiter.tryRemove(), true);
});

test('msUntil agrees with tryRemove', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 0.3, now: clock.now });
  limiter.tryRemove(2);
  const wait = limiter.msUntil(2);
  clock.advance(wait - 1);
  assert.strictEqual(limiter.tryRemove(2), false);
  clock.advance(1);
  assert.strictEqual(limiter.tryRemove(2), true);
});

test('reset makes the bucket full again', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(3);
  limiter.reset();
  assert.strictEqual(limiter.available(), 3);
  assert.strictEqual(limiter.tryRemove(3), true);
});

test('clock going backwards adds and loses no tokens', () => {
  const clock = makeClock(10_000);
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.available(), 5);

  clock.time = 5_000;
  assert.strictEqual(limiter.available(), 5);
  limiter.tryRemove(5);

  clock.time = 4_000;
  assert.strictEqual(limiter.available(), 0);

  // The earlier time is the reference, so refill counts from 4000.
  clock.time = 5_000;
  assert.strictEqual(limiter.available(), 1);
});
