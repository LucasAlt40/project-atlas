'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const lib = require('../src/limiter');
const { createLimiter } = lib;

function fakeClock(start = 0) {
  const clock = {
    t: start,
    now: () => clock.t,
    advance: (ms) => {
      clock.t += ms;
    },
  };
  return clock;
}

test('exports only createLimiter', () => {
  assert.deepStrictEqual(Object.keys(lib), ['createLimiter']);
});

test('bucket starts full', () => {
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: fakeClock().now });
  assert.strictEqual(limiter.available(), 5);
});

test('works with the default clock', () => {
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1 });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.available(), 1);
});

test('tryRemove removes tokens until the bucket is empty', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), false);
  assert.strictEqual(limiter.available(), 0);
});

test('tryRemove(n) is all or nothing', () => {
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: fakeClock().now });
  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.tryRemove(3), false);
  assert.strictEqual(limiter.available(), 2);
  assert.strictEqual(limiter.tryRemove(2), true);
  assert.strictEqual(limiter.available(), 0);
});

test('tokens refill continuously with elapsed time', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 2, now: clock.now });
  limiter.tryRemove(10);
  clock.advance(1000);
  assert.strictEqual(limiter.available(), 2);
  clock.advance(250);
  assert.strictEqual(limiter.available(), 2);
  clock.advance(250);
  assert.strictEqual(limiter.available(), 3);
});

test('fractional refill rates accumulate across calls', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 0.5, now: clock.now });
  limiter.tryRemove(4);
  clock.advance(1000);
  assert.strictEqual(limiter.available(), 0);
  clock.advance(1000);
  assert.strictEqual(limiter.available(), 1);
  clock.advance(4000);
  assert.strictEqual(limiter.available(), 3);
});

test('refill never exceeds capacity', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(1);
  clock.advance(1000000);
  assert.strictEqual(limiter.available(), 4);
  assert.strictEqual(limiter.tryRemove(4), true);
  assert.strictEqual(limiter.tryRemove(), false);
});

test('available rounds fractional tokens down', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(3);
  clock.advance(1500);
  assert.strictEqual(limiter.available(), 3);
  clock.advance(500);
  assert.strictEqual(limiter.available(), 4);
});

test('tryRemove counts fractional tokens only once whole', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(2);
  clock.advance(500);
  assert.strictEqual(limiter.tryRemove(), false);
  clock.advance(500);
  assert.strictEqual(limiter.tryRemove(), true);
});

test('msUntil is 0 when tokens are already available', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
  assert.strictEqual(limiter.msUntil(), 0);
  assert.strictEqual(limiter.msUntil(3), 0);
});

test('msUntil returns the milliseconds until enough tokens exist', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 2, now: clock.now });
  limiter.tryRemove(5);
  assert.strictEqual(limiter.msUntil(), 500);
  assert.strictEqual(limiter.msUntil(3), 1500);
  clock.advance(500);
  assert.strictEqual(limiter.msUntil(), 0);
});

test('msUntil rounds up to a whole millisecond', () => {
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 3, now: fakeClock().now });
  limiter.tryRemove();
  assert.strictEqual(limiter.msUntil(), 334);
});

test('msUntil accounts for fractional tokens already present', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(2);
  clock.advance(500);
  assert.strictEqual(limiter.msUntil(), 500);
});

test('msUntil does not consume tokens', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: clock.now });
  limiter.msUntil(2);
  assert.strictEqual(limiter.available(), 2);
});

test('reset makes the bucket full again', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 0.001, now: fakeClock().now });
  limiter.tryRemove(3);
  assert.strictEqual(limiter.available(), 0);
  limiter.reset();
  assert.strictEqual(limiter.available(), 3);
});

test('a backwards clock adds no tokens and loses none', () => {
  const clock = fakeClock(10000);
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(2);
  clock.t = 9000;
  assert.strictEqual(limiter.available(), 3);
  clock.t = 10000;
  assert.strictEqual(limiter.available(), 4);
});

test('the earlier time becomes the new reference after the clock goes backwards', () => {
  const clock = fakeClock(10000);
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(5);
  clock.t = 5000;
  assert.strictEqual(limiter.available(), 0);
  clock.t = 6000;
  assert.strictEqual(limiter.available(), 1);
});

test('invalid capacity throws RangeError', () => {
  const now = fakeClock().now;
  for (const capacity of [undefined, null, '5', 0, -1, 1.5, NaN, Infinity]) {
    assert.throws(() => createLimiter({ capacity, refillPerSecond: 1, now }), RangeError, `capacity ${capacity}`);
  }
});

test('invalid refillPerSecond throws RangeError', () => {
  const now = fakeClock().now;
  for (const refillPerSecond of [undefined, null, '1', 0, -1, NaN, Infinity, -Infinity]) {
    assert.throws(() => createLimiter({ capacity: 1, refillPerSecond, now }), RangeError, `refill ${refillPerSecond}`);
  }
});

test('missing options throw RangeError', () => {
  assert.throws(() => createLimiter(), RangeError);
});

test('invalid n throws RangeError from tryRemove and msUntil', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
  for (const n of [0, -1, 1.5, '1', NaN, Infinity, null]) {
    assert.throws(() => limiter.tryRemove(n), RangeError, `tryRemove(${n})`);
    assert.throws(() => limiter.msUntil(n), RangeError, `msUntil(${n})`);
  }
});

test('n greater than capacity throws RangeError', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
  assert.throws(() => limiter.tryRemove(4), RangeError);
  assert.throws(() => limiter.msUntil(4), RangeError);
  assert.strictEqual(limiter.available(), 3);
});
