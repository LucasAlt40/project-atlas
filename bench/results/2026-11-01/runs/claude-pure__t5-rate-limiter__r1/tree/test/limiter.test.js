'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const { createLimiter } = require('../src/limiter');

// A clock the tests move by hand, so refill timing is exact.
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

test('exports createLimiter only', () => {
  assert.deepStrictEqual(Object.keys(require('../src/limiter')), ['createLimiter']);
});

test('rejects invalid capacity', () => {
  const bad = [undefined, null, '5', 0, -1, 1.5, NaN, Infinity, -Infinity];
  for (const capacity of bad) {
    assert.throws(() => createLimiter({ capacity, refillPerSecond: 1 }), RangeError, `capacity ${capacity}`);
  }
});

test('rejects invalid refillPerSecond', () => {
  const bad = [undefined, null, '1', 0, -1, NaN, Infinity, -Infinity];
  for (const refillPerSecond of bad) {
    assert.throws(() => createLimiter({ capacity: 5, refillPerSecond }), RangeError, `refill ${refillPerSecond}`);
  }
});

test('accepts fractional refillPerSecond', () => {
  assert.doesNotThrow(() => createLimiter({ capacity: 1, refillPerSecond: 0.5 }));
});

test('starts full', () => {
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 1, now: fakeClock().now });
  assert.strictEqual(limiter.available(), 4);
});

test('tryRemove takes tokens and returns false without taking when short', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
  assert.strictEqual(limiter.tryRemove(2), true);
  assert.strictEqual(limiter.available(), 1);
  assert.strictEqual(limiter.tryRemove(2), false);
  assert.strictEqual(limiter.available(), 1);
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), false);
});

test('tryRemove and msUntil reject invalid n', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
  const bad = [0, -1, 1.5, NaN, Infinity, '1', null];
  for (const n of bad) {
    assert.throws(() => limiter.tryRemove(n), RangeError, `tryRemove(${n})`);
    assert.throws(() => limiter.msUntil(n), RangeError, `msUntil(${n})`);
  }
});

test('tryRemove and msUntil reject n greater than capacity', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
  assert.throws(() => limiter.tryRemove(4), RangeError);
  assert.throws(() => limiter.msUntil(4), RangeError);
});

test('refills continuously and floors available()', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 2, now: clock.now });
  assert.strictEqual(limiter.tryRemove(10), true);
  assert.strictEqual(limiter.available(), 0);

  clock.advance(500);
  assert.strictEqual(limiter.available(), 1);

  clock.advance(250);
  assert.strictEqual(limiter.available(), 1);
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), false);
});

test('fractional refill rate adds whole tokens only after enough time', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 0.5, now: clock.now });
  assert.strictEqual(limiter.tryRemove(2), true);

  clock.advance(1999);
  assert.strictEqual(limiter.available(), 0);
  clock.advance(1);
  assert.strictEqual(limiter.available(), 1);
});

test('refill never exceeds capacity', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 10, now: clock.now });
  assert.strictEqual(limiter.tryRemove(1), true);

  clock.advance(60_000);
  assert.strictEqual(limiter.available(), 5);
});

test('msUntil is 0 when tokens are already available', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
  assert.strictEqual(limiter.msUntil(), 0);
  assert.strictEqual(limiter.msUntil(3), 0);
});

test('msUntil counts time for the missing tokens, rounded up', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 3, now: clock.now });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.msUntil(), 334); // ceil(1000 / 3)
});

test('msUntil accounts for partial refill', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 2, now: clock.now });
  assert.strictEqual(limiter.tryRemove(4), true);

  clock.advance(250); // 0.5 tokens accumulated
  assert.strictEqual(limiter.msUntil(1), 250);
  assert.strictEqual(limiter.msUntil(2), 750); // 1.5 missing tokens at 2/s
});

test('msUntil with a fractional rate', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 0.5, now: clock.now });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.msUntil(), 2000);
});

test('tryRemove succeeds once the time reported by msUntil has passed', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 3, now: clock.now });
  assert.strictEqual(limiter.tryRemove(5), true);

  const wait = limiter.msUntil(4);
  assert.strictEqual(limiter.tryRemove(4), false);
  clock.advance(wait);
  assert.strictEqual(limiter.tryRemove(4), true);
});

test('reset makes the bucket full again', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(3), true);
  limiter.reset();
  assert.strictEqual(limiter.available(), 3);
  assert.strictEqual(limiter.tryRemove(3), true);
});

test('clock going backwards adds no tokens and loses none', () => {
  const clock = fakeClock(10_000);
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(5), true);

  clock.set(0); // earlier than the last time seen
  assert.strictEqual(limiter.available(), 5);

  clock.advance(1000); // 1 second after the new reference
  assert.strictEqual(limiter.available(), 6);
});
