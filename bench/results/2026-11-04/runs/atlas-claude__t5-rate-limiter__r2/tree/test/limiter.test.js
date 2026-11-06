const { test } = require('node:test');
const assert = require('node:assert');
const { createLimiter } = require('../src/limiter');

// Fake clock: tests move `time` forward or backward by hand.
function fakeClock(start = 0) {
  const clock = {
    time: start,
    now: () => clock.time,
    advance: (ms) => {
      clock.time += ms;
    },
  };
  return clock;
}

test('createLimiter validates capacity and refillPerSecond', () => {
  const bad = [undefined, null, 0, -1, 1.5, '5', NaN, Infinity];
  for (const capacity of bad) {
    assert.throws(() => createLimiter({ capacity, refillPerSecond: 1 }), RangeError, `capacity ${capacity}`);
  }
  const badRefill = [undefined, 0, -1, NaN, Infinity, '1'];
  for (const refillPerSecond of badRefill) {
    assert.throws(() => createLimiter({ capacity: 1, refillPerSecond }), RangeError, `refill ${refillPerSecond}`);
  }
});

test('createLimiter accepts fractional refillPerSecond', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 0.5, now: clock.now });
  assert.strictEqual(limiter.tryRemove(), true);
});

test('bucket starts full', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
  assert.strictEqual(limiter.available(), 3);
});

test('tryRemove removes tokens when available and fails without removing otherwise', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(2), true);
  assert.strictEqual(limiter.tryRemove(2), false);
  assert.strictEqual(limiter.available(), 1);
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), false);
});

test('tokens refill continuously with elapsed time', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(2), true);
  clock.advance(500);
  assert.strictEqual(limiter.available(), 0);
  clock.advance(500);
  assert.strictEqual(limiter.available(), 1);
  assert.strictEqual(limiter.tryRemove(), true);
});

test('fractional refill accumulates across calls', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 0.5, now: clock.now });
  assert.strictEqual(limiter.tryRemove(2), true);
  clock.advance(1000);
  assert.strictEqual(limiter.tryRemove(), false);
  clock.advance(1000);
  assert.strictEqual(limiter.tryRemove(), true);
});

test('tokens never grow above capacity', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: clock.now });
  clock.advance(1_000_000);
  assert.strictEqual(limiter.available(), 2);
  assert.strictEqual(limiter.tryRemove(2), true);
  assert.strictEqual(limiter.tryRemove(1), false);
});

test('available rounds fractional tokens down', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 0.5, now: clock.now });
  assert.strictEqual(limiter.tryRemove(3), true);
  clock.advance(1500);
  assert.strictEqual(limiter.available(), 0);
  clock.advance(500);
  assert.strictEqual(limiter.available(), 1);
});

test('n is validated: positive integer and not above capacity', () => {
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: fakeClock().now });
  for (const n of [0, -1, 1.5, NaN, '1', null]) {
    assert.throws(() => limiter.tryRemove(n), RangeError, `tryRemove(${n})`);
    assert.throws(() => limiter.msUntil(n), RangeError, `msUntil(${n})`);
  }
  assert.throws(() => limiter.tryRemove(3), RangeError);
  assert.throws(() => limiter.msUntil(3), RangeError);
});

test('msUntil is 0 when tokens are available', () => {
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: fakeClock().now });
  assert.strictEqual(limiter.msUntil(), 0);
  assert.strictEqual(limiter.msUntil(2), 0);
});

test('msUntil returns whole milliseconds rounded up', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(2), true);
  assert.strictEqual(limiter.msUntil(), 1000);
  assert.strictEqual(limiter.msUntil(2), 2000);
  clock.advance(300);
  assert.strictEqual(limiter.msUntil(), 700);

  const fast = createLimiter({ capacity: 1, refillPerSecond: 3, now: clock.now });
  assert.strictEqual(fast.tryRemove(), true);
  assert.strictEqual(fast.msUntil(), 334);
});

test('msUntil with fractional refill', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 0.5, now: clock.now });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.msUntil(), 2000);
  clock.advance(1000);
  assert.strictEqual(limiter.msUntil(), 1000);
});

test('floating-point drift does not cause off-by-one results', () => {
  // Ten refills of 100ms at 1 token/s sum to 0.9999999999999999 in floating point.
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(2), true);
  for (let i = 0; i < 10; i++) {
    clock.advance(100);
    limiter.available();
  }
  assert.strictEqual(limiter.available(), 1);
  assert.strictEqual(limiter.msUntil(), 0);
  assert.strictEqual(limiter.tryRemove(), true);
});

test('a backwards clock adds no tokens and loses none', () => {
  const clock = fakeClock(10_000);
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(5), true);
  clock.time = 0;
  assert.strictEqual(limiter.available(), 0);
  // The earlier time is now the reference, so 1s forward yields one token.
  clock.time = 1000;
  assert.strictEqual(limiter.available(), 1);
});

test('reset makes the bucket full again', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(3), true);
  limiter.reset();
  assert.strictEqual(limiter.available(), 3);
  assert.strictEqual(limiter.tryRemove(3), true);
});

test('now defaults to Date.now', () => {
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 1 });
  assert.strictEqual(limiter.available(), 1);
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), false);
});
