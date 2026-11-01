const test = require('node:test');
const assert = require('node:assert');
const { createLimiter } = require('../src/limiter');

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

test('starts full', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.available(), 3);
  assert.strictEqual(limiter.msUntil(), 0);
});

test('tryRemove returns false when empty and removes nothing on failure', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), false);
  assert.strictEqual(limiter.available(), 0);
});

test('tryRemove(n) removes n tokens or nothing at all', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.available(), 2);
  assert.strictEqual(limiter.tryRemove(3), false);
  assert.strictEqual(limiter.available(), 2);
});

test('tokens refill over time and never exceed capacity', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(2);
  clock.advance(999);
  assert.strictEqual(limiter.available(), 0);
  clock.advance(1);
  assert.strictEqual(limiter.available(), 1);
  clock.advance(10000);
  assert.strictEqual(limiter.available(), 2);
});

test('fractional refill rate accumulates across calls', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 0.5, now: clock.now });
  assert.strictEqual(limiter.tryRemove(), true);
  clock.advance(1000);
  assert.strictEqual(limiter.tryRemove(), false);
  clock.advance(1000);
  assert.strictEqual(limiter.tryRemove(), true);
});

test('msUntil returns the time until n tokens are available', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 2, now: clock.now });
  limiter.tryRemove(3);
  assert.strictEqual(limiter.msUntil(), 500);
  assert.strictEqual(limiter.msUntil(2), 1000);
  assert.strictEqual(limiter.msUntil(3), 1500);
  clock.advance(250);
  assert.strictEqual(limiter.msUntil(), 250);
  clock.advance(250);
  assert.strictEqual(limiter.msUntil(), 0);
});

test('msUntil rounds up to whole milliseconds', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 3, now: clock.now });
  limiter.tryRemove();
  assert.strictEqual(limiter.msUntil(), 334);
});

test('reset makes the bucket full again', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(4);
  assert.strictEqual(limiter.available(), 0);
  limiter.reset();
  assert.strictEqual(limiter.available(), 4);
});

test('clock going backwards adds no tokens and loses none', () => {
  const clock = fakeClock(1000);
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove();
  clock.set(400);
  assert.strictEqual(limiter.available(), 1);
  clock.advance(1000);
  assert.strictEqual(limiter.available(), 2);
});

test('float error does not shift floor or ceil by one token', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 0.1, now: clock.now });
  limiter.tryRemove(5);
  // Ten refills of 0.1 token each sum to 0.9999999999999999 in floating point.
  for (let i = 0; i < 10; i++) {
    clock.advance(1000);
    limiter.available();
  }
  assert.strictEqual(limiter.available(), 1);
  assert.strictEqual(limiter.msUntil(), 0);
  assert.strictEqual(limiter.tryRemove(), true);
});

test('uses Date.now by default', () => {
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 1 });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), false);
});

test('rejects invalid capacity and refillPerSecond with RangeError', () => {
  const invalidCapacities = [undefined, 0, -1, 1.5, NaN, Infinity, '3'];
  for (const capacity of invalidCapacities) {
    assert.throws(() => createLimiter({ capacity, refillPerSecond: 1 }), RangeError);
  }

  const invalidRates = [undefined, 0, -1, NaN, Infinity, '1'];
  for (const refillPerSecond of invalidRates) {
    assert.throws(() => createLimiter({ capacity: 1, refillPerSecond }), RangeError);
  }
});

test('accepts a fractional refillPerSecond', () => {
  assert.doesNotThrow(() => createLimiter({ capacity: 1, refillPerSecond: 0.5 }));
});

test('rejects invalid n in tryRemove and msUntil with RangeError', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1 });
  const invalidCounts = [0, -1, 1.5, NaN, 4];
  for (const n of invalidCounts) {
    assert.throws(() => limiter.tryRemove(n), RangeError);
    assert.throws(() => limiter.msUntil(n), RangeError);
  }
});
