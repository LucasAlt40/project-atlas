const { test } = require('node:test');
const assert = require('node:assert');
const { createLimiter } = require('../src/limiter');

// Fake clock: time only moves when the test advances it.
function fakeClock(start = 0) {
  let t = start;
  return {
    now: () => t,
    advance: (ms) => {
      t += ms;
    },
    set: (ms) => {
      t = ms;
    },
  };
}

test('exports createLimiter', () => {
  assert.strictEqual(typeof createLimiter, 'function');
});

test('starts full', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.available(), 3);
});

test('works with the default clock when now is omitted', () => {
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1 });
  assert.strictEqual(limiter.available(), 2);
  assert.strictEqual(limiter.tryRemove(), true);
});

test('tryRemove succeeds until the bucket is empty, then fails', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), false);
});

test('tryRemove(n) removes n tokens at once, or nothing when short', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.available(), 2);
  assert.strictEqual(limiter.tryRemove(3), false);
  assert.strictEqual(limiter.available(), 2);
});

test('refills continuously with elapsed time', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(2);
  clock.advance(500);
  assert.strictEqual(limiter.available(), 0);
  clock.advance(500);
  assert.strictEqual(limiter.available(), 1);
  clock.advance(1000);
  assert.strictEqual(limiter.available(), 2);
});

test('tokens never grow above capacity', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 10, now: clock.now });
  limiter.tryRemove(1);
  clock.advance(60000);
  assert.strictEqual(limiter.available(), 2);
});

test('fractional refill rate accumulates without off-by-one at integer boundaries', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 0.3, now: clock.now });
  limiter.tryRemove(10);
  for (let i = 0; i < 10; i++) {
    clock.advance(1000);
    limiter.available();
  }
  // 10 seconds at 0.3/s is exactly 3 tokens.
  assert.strictEqual(limiter.available(), 3);
  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.available(), 0);
});

test('msUntil is 0 when tokens are already available', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.msUntil(), 0);
  assert.strictEqual(limiter.msUntil(2), 0);
});

test('msUntil returns the milliseconds until n tokens are available', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 0.5, now: clock.now });
  limiter.tryRemove(2);
  assert.strictEqual(limiter.msUntil(), 2000);
  assert.strictEqual(limiter.msUntil(2), 4000);
  clock.advance(500);
  assert.strictEqual(limiter.msUntil(), 1500);
});

test('msUntil rounds up to a whole millisecond', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 3, now: clock.now });
  limiter.tryRemove(1);
  // 1 token at 3/s is 333.33... ms, rounded up.
  assert.strictEqual(limiter.msUntil(), 334);
});

test('msUntil agrees with tryRemove: waiting that long makes the removal succeed', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 0.3, now: clock.now });
  limiter.tryRemove(4);
  const wait = limiter.msUntil(2);
  assert.strictEqual(limiter.tryRemove(2), false);
  clock.advance(wait);
  assert.strictEqual(limiter.tryRemove(2), true);
});

test('reset makes the bucket full again', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(3);
  limiter.reset();
  assert.strictEqual(limiter.available(), 3);
});

test('clock going backwards adds no tokens', () => {
  const clock = fakeClock(5000);
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(5);
  clock.set(1000);
  assert.strictEqual(limiter.available(), 0);
});

test('clock going backwards loses no tokens and resets the reference', () => {
  const clock = fakeClock(10000);
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(3);
  clock.set(9000);
  assert.strictEqual(limiter.available(), 2);
  // Reference is now 9000, so one second later one token has been added.
  clock.set(10000);
  assert.strictEqual(limiter.available(), 3);
});

test('createLimiter rejects an invalid capacity', () => {
  for (const capacity of [undefined, 0, -1, 1.5, '2', NaN, Infinity, null]) {
    assert.throws(() => createLimiter({ capacity, refillPerSecond: 1 }), RangeError, `capacity ${capacity}`);
  }
});

test('createLimiter rejects an invalid refillPerSecond', () => {
  for (const refillPerSecond of [undefined, 0, -1, NaN, Infinity, '1', null]) {
    assert.throws(() => createLimiter({ capacity: 1, refillPerSecond }), RangeError, `refillPerSecond ${refillPerSecond}`);
  }
});

test('createLimiter accepts a fractional refillPerSecond', () => {
  assert.doesNotThrow(() => createLimiter({ capacity: 1, refillPerSecond: 0.5 }));
});

test('createLimiter rejects a now that is not a function', () => {
  assert.throws(() => createLimiter({ capacity: 1, refillPerSecond: 1, now: 123 }), TypeError);
});

test('tryRemove and msUntil reject an invalid n', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
  for (const n of [0, -1, 1.5, '1', NaN, null]) {
    assert.throws(() => limiter.tryRemove(n), RangeError, `tryRemove ${n}`);
    assert.throws(() => limiter.msUntil(n), RangeError, `msUntil ${n}`);
  }
});

test('tryRemove and msUntil reject n greater than capacity', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
  assert.throws(() => limiter.tryRemove(4), RangeError);
  assert.throws(() => limiter.msUntil(4), RangeError);
});
