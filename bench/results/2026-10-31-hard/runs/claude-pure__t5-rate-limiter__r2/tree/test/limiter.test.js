const { test } = require('node:test');
const assert = require('node:assert');
const limiterModule = require('../src/limiter');
const { createLimiter } = limiterModule;

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

test('exports only createLimiter', () => {
  assert.deepStrictEqual(Object.keys(limiterModule), ['createLimiter']);
});

test('works with the default clock when now is omitted', () => {
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1 });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.available(), 1);
});

test('starts full', () => {
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 1, now: fakeClock().now });
  assert.strictEqual(limiter.available(), 4);
});

test('tryRemove removes tokens until the bucket is empty', () => {
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: fakeClock().now });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), false);
  assert.strictEqual(limiter.available(), 0);
});

test('tryRemove(n) takes n tokens or nothing', () => {
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: fakeClock().now });
  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.tryRemove(3), false);
  assert.strictEqual(limiter.available(), 2);
  assert.strictEqual(limiter.tryRemove(2), true);
  assert.strictEqual(limiter.available(), 0);
});

test('refills continuously at refillPerSecond', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(2);
  clock.advance(500);
  assert.strictEqual(limiter.available(), 0);
  clock.advance(500);
  assert.strictEqual(limiter.available(), 1);
});

test('supports fractional refill rates', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 0.5, now: clock.now });
  limiter.tryRemove(3);
  clock.advance(1000);
  assert.strictEqual(limiter.available(), 0);
  clock.advance(1000);
  assert.strictEqual(limiter.available(), 1);
});

test('never refills above capacity', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(1);
  clock.advance(1_000_000);
  assert.strictEqual(limiter.available(), 3);
  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.tryRemove(), false);
});

test('a failed tryRemove removes nothing', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(1);
  assert.strictEqual(limiter.tryRemove(2), false);
  assert.strictEqual(limiter.available(), 1);
});

test('available is rounded down', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(5);
  clock.advance(2999);
  assert.strictEqual(limiter.available(), 2);
});

test('msUntil is 0 when tokens are already available', () => {
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: fakeClock().now });
  assert.strictEqual(limiter.msUntil(), 0);
  assert.strictEqual(limiter.msUntil(2), 0);
});

test('msUntil counts down to the next token', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 2, now: clock.now });
  limiter.tryRemove();
  assert.strictEqual(limiter.msUntil(), 500);
  clock.advance(250);
  assert.strictEqual(limiter.msUntil(), 250);
  clock.advance(250);
  assert.strictEqual(limiter.msUntil(), 0);
});

test('msUntil rounds up to whole milliseconds', () => {
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 3, now: fakeClock().now });
  limiter.tryRemove();
  assert.strictEqual(limiter.msUntil(), 334);
});

test('msUntil(n) accounts for the tokens still missing', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(4);
  assert.strictEqual(limiter.msUntil(3), 3000);
  clock.advance(1000);
  assert.strictEqual(limiter.msUntil(3), 2000);
});

test('reset makes the bucket full again', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
  limiter.tryRemove(3);
  limiter.reset();
  assert.strictEqual(limiter.available(), 3);
  assert.strictEqual(limiter.tryRemove(3), true);
});

test('a clock moving backwards neither adds nor loses tokens', () => {
  const clock = fakeClock(1000);
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(2);
  clock.set(0);
  assert.strictEqual(limiter.available(), 1);
  clock.set(1000);
  assert.strictEqual(limiter.available(), 2);
});

test('createLimiter rejects an invalid capacity', () => {
  const bad = [undefined, 0, -1, 1.5, NaN, Infinity, '3', null];
  for (const capacity of bad) {
    assert.throws(() => createLimiter({ capacity, refillPerSecond: 1 }), RangeError, `capacity ${capacity}`);
  }
  assert.throws(() => createLimiter(), RangeError);
});

test('createLimiter rejects an invalid refillPerSecond', () => {
  const bad = [undefined, 0, -1, NaN, Infinity, '1', null];
  for (const refillPerSecond of bad) {
    assert.throws(() => createLimiter({ capacity: 1, refillPerSecond }), RangeError, `refill ${refillPerSecond}`);
  }
});

test('tryRemove and msUntil reject invalid n', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
  for (const n of [0, -1, 1.5, NaN, '1', null]) {
    assert.throws(() => limiter.tryRemove(n), RangeError, `tryRemove ${n}`);
    assert.throws(() => limiter.msUntil(n), RangeError, `msUntil ${n}`);
  }
});

test('tryRemove and msUntil reject n greater than capacity', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
  assert.throws(() => limiter.tryRemove(4), RangeError);
  assert.throws(() => limiter.msUntil(4), RangeError);
});
