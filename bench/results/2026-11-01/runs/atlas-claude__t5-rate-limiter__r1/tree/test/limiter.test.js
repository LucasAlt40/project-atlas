'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const { createLimiter } = require('../src/limiter');

// Injectable clock: `clock.t` is the current time in ms.
function makeClock(start = 0) {
  const clock = { t: start };
  clock.now = () => clock.t;
  return clock;
}

test('rejects invalid capacity', () => {
  for (const capacity of [undefined, 0, -1, 1.5, '3', NaN, Infinity]) {
    assert.throws(
      () => createLimiter({ capacity, refillPerSecond: 1 }),
      RangeError,
      `capacity ${String(capacity)} should be rejected`
    );
  }
  assert.throws(() => createLimiter(), RangeError);
});

test('rejects invalid refillPerSecond', () => {
  for (const refillPerSecond of [undefined, 0, -1, '1', NaN, Infinity]) {
    assert.throws(
      () => createLimiter({ capacity: 5, refillPerSecond }),
      RangeError,
      `refillPerSecond ${String(refillPerSecond)} should be rejected`
    );
  }
});

test('accepts fractional refillPerSecond', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 0.5, now: clock.now });
  assert.strictEqual(limiter.tryRemove(), true);
  clock.t = 1000;
  assert.strictEqual(limiter.available(), 0);
  clock.t = 2000;
  assert.strictEqual(limiter.available(), 1);
});

test('rejects a now option that is not a function', () => {
  assert.throws(() => createLimiter({ capacity: 1, refillPerSecond: 1, now: 5 }), TypeError);
});

test('rejects a clock reading that is not finite', () => {
  assert.throws(
    () => createLimiter({ capacity: 1, refillPerSecond: 1, now: () => NaN }),
    TypeError
  );

  const clock = makeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: clock.now });
  clock.t = Infinity;
  assert.throws(() => limiter.available(), TypeError);
  assert.throws(() => limiter.tryRemove(), TypeError);

  // The failed reads did not corrupt state: the bucket still works afterwards.
  clock.t = 0;
  assert.strictEqual(limiter.available(), 2);
});

test('defaults to Date.now when now is not given', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1 });
  assert.strictEqual(limiter.available(), 3);
  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.available(), 0);
});

test('starts full', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.available(), 4);
});

test('tryRemove defaults to one token and fails when empty', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), false);
  assert.strictEqual(limiter.available(), 0);
});

test('tryRemove(n) removes nothing when fewer than n tokens are available', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.tryRemove(3), false);
  assert.strictEqual(limiter.available(), 2);
});

test('tokens refill continuously and never exceed capacity', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 2, now: clock.now });
  assert.strictEqual(limiter.tryRemove(10), true);

  clock.t = 1000;
  assert.strictEqual(limiter.available(), 2);

  clock.t = 1000 + 60 * 60 * 1000;
  assert.strictEqual(limiter.available(), 10);
});

test('partial refill accumulates across calls', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(3), true);

  clock.t = 500;
  assert.strictEqual(limiter.available(), 0);
  clock.t = 999;
  assert.strictEqual(limiter.available(), 0);
  clock.t = 1000;
  assert.strictEqual(limiter.available(), 1);
});

test('exact refill boundaries are not missed to floating-point error', () => {
  const clock = makeClock();
  // 0.1 tokens/s: one token every 10000 ms.
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 0.1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(), true);

  clock.t = 9999;
  assert.strictEqual(limiter.tryRemove(), false);
  clock.t = 10000;
  assert.strictEqual(limiter.tryRemove(), true);
});

test('msUntil returns 0 when tokens are already available', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.msUntil(), 0);
  assert.strictEqual(limiter.msUntil(3), 0);
});

test('msUntil returns whole milliseconds rounded up', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 4, now: clock.now });
  assert.strictEqual(limiter.tryRemove(10), true);
  assert.strictEqual(limiter.msUntil(), 250);
  assert.strictEqual(limiter.msUntil(2), 500);

  const slow = createLimiter({ capacity: 1, refillPerSecond: 3, now: clock.now });
  assert.strictEqual(slow.tryRemove(), true);
  assert.strictEqual(slow.msUntil(), 334);
});

test('waiting msUntil(n) ms makes tryRemove(n) succeed', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 7, refillPerSecond: 0.3, now: clock.now });
  assert.strictEqual(limiter.tryRemove(7), true);

  const wait = limiter.msUntil(4);
  clock.t += wait - 1;
  assert.strictEqual(limiter.tryRemove(4), false);
  clock.t += 1;
  assert.strictEqual(limiter.tryRemove(4), true);
});

test('tryRemove and msUntil reject invalid n', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  for (const n of [0, -1, 1.5, '1', NaN, Infinity]) {
    assert.throws(() => limiter.tryRemove(n), RangeError, `tryRemove(${String(n)})`);
    assert.throws(() => limiter.msUntil(n), RangeError, `msUntil(${String(n)})`);
  }
});

test('tryRemove and msUntil reject n greater than capacity', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  assert.throws(() => limiter.tryRemove(6), RangeError);
  assert.throws(() => limiter.msUntil(6), RangeError);
});

test('reset makes the bucket full again', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(4), true);
  limiter.reset();
  assert.strictEqual(limiter.available(), 4);
});

test('a clock that goes backwards neither adds nor loses tokens', () => {
  const clock = makeClock(1000);
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(10), true);

  clock.t = 5000;
  assert.strictEqual(limiter.available(), 4);

  clock.t = 0;
  assert.strictEqual(limiter.available(), 4);

  // The earlier time 0 is now the reference, so 1 s later one token is added.
  clock.t = 1000;
  assert.strictEqual(limiter.available(), 5);
});
