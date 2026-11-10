const { test } = require('node:test');
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
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.available(), 5);
});

test('tryRemove takes n tokens and reports success', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.available(), 2);
});

test('tryRemove defaults to one token', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.available(), 1);
});

test('tryRemove returns false and removes nothing when not enough tokens', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.tryRemove(3), false);
  assert.strictEqual(limiter.available(), 2);
});

test('refills continuously and never above capacity', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 2, now: clock.now });
  assert.strictEqual(limiter.tryRemove(10), true);
  clock.advance(500);
  assert.strictEqual(limiter.available(), 1);
  clock.advance(250);
  assert.strictEqual(limiter.available(), 1);
  clock.advance(250);
  assert.strictEqual(limiter.available(), 2);
  clock.advance(10 * 60 * 1000);
  assert.strictEqual(limiter.available(), 10);
});

test('fractional refill rate adds tokens over time', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 0.5, now: clock.now });
  assert.strictEqual(limiter.tryRemove(2), true);
  clock.advance(1000);
  assert.strictEqual(limiter.available(), 0);
  clock.advance(1000);
  assert.strictEqual(limiter.available(), 1);
});

test('tryRemove succeeds once msUntil has elapsed, despite float drift', () => {
  for (const refillPerSecond of [0.1, 0.3, 0.7, 1 / 3]) {
    const clock = fakeClock();
    const limiter = createLimiter({ capacity: 1, refillPerSecond, now: clock.now });
    assert.strictEqual(limiter.tryRemove(1), true);
    const wait = limiter.msUntil(1);
    assert.ok(wait > 0);
    clock.advance(wait - 1);
    assert.strictEqual(limiter.tryRemove(1), false, `too early at rate ${refillPerSecond}`);
    clock.advance(1);
    assert.strictEqual(limiter.tryRemove(1), true, `too late at rate ${refillPerSecond}`);
  }
});

test('fractional refills accumulated in small steps still count as whole tokens', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(10), true);
  for (let i = 0; i < 10; i++) {
    clock.advance(100);
    limiter.available();
  }
  // Ten additions of 0.1 sum to 0.9999999999999999 in floating point.
  assert.strictEqual(limiter.msUntil(1), 0);
  assert.strictEqual(limiter.tryRemove(1), true);
});

test('available counts accumulated fractional refills as whole tokens', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(10), true);
  for (let i = 0; i < 10; i++) {
    clock.advance(100);
    limiter.available();
  }
  assert.strictEqual(limiter.available(), 1);
});

test('msUntil is 0 when tokens are already available', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.msUntil(5), 0);
  assert.strictEqual(limiter.msUntil(), 0);
});

test('msUntil rounds up to whole milliseconds', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 3, now: clock.now });
  assert.strictEqual(limiter.tryRemove(10), true);
  assert.strictEqual(limiter.msUntil(1), 334);
});

test('msUntil counts the deficit for n tokens', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 0.5, now: clock.now });
  assert.strictEqual(limiter.tryRemove(2), true);
  assert.strictEqual(limiter.msUntil(1), 2000);
  assert.strictEqual(limiter.msUntil(2), 4000);
});

test('reset makes the bucket full again', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(4), true);
  limiter.reset();
  assert.strictEqual(limiter.available(), 4);
});

test('reset after the clock went backwards makes the bucket full', () => {
  const clock = fakeClock(10000);
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(5), true);
  clock.set(5000);
  limiter.reset();
  assert.strictEqual(limiter.available(), 5);
  clock.set(5500);
  assert.strictEqual(limiter.available(), 5);
});

test('clock going backwards adds no tokens and removes none', () => {
  const clock = fakeClock(10000);
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(2), true);
  clock.set(9000);
  assert.strictEqual(limiter.available(), 3);
});

test('after clock goes backwards, the earlier time is the new reference', () => {
  const clock = fakeClock(10000);
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(5), true);
  clock.set(5000);
  assert.strictEqual(limiter.available(), 0);
  clock.set(6000);
  assert.strictEqual(limiter.available(), 1);
});

test('createLimiter rejects invalid capacity', () => {
  const now = fakeClock().now;
  for (const capacity of [undefined, 0, -1, 1.5, '5', NaN, Infinity]) {
    assert.throws(() => createLimiter({ capacity, refillPerSecond: 1, now }), RangeError, `capacity ${capacity}`);
  }
});

test('createLimiter rejects invalid refillPerSecond', () => {
  const now = fakeClock().now;
  for (const refillPerSecond of [undefined, 0, -1, '1', NaN, Infinity]) {
    assert.throws(() => createLimiter({ capacity: 1, refillPerSecond, now }), RangeError, `refill ${refillPerSecond}`);
  }
});

test('createLimiter rejects null options with a RangeError', () => {
  assert.throws(() => createLimiter(null), RangeError);
});

test('createLimiter uses Date.now when now is not given', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1 });
  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.tryRemove(1), false);
  limiter.reset();
  assert.strictEqual(limiter.available(), 3);
});

test('createLimiter rejects a now that is not a function', () => {
  assert.throws(() => createLimiter({ capacity: 1, refillPerSecond: 1, now: 123 }), TypeError);
});

test('tryRemove and msUntil reject invalid n', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  for (const n of [0, -1, 1.5, NaN, '1']) {
    assert.throws(() => limiter.tryRemove(n), RangeError, `tryRemove ${n}`);
    assert.throws(() => limiter.msUntil(n), RangeError, `msUntil ${n}`);
  }
});

test('tryRemove and msUntil reject n greater than capacity', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  assert.throws(() => limiter.tryRemove(6), RangeError);
  assert.throws(() => limiter.msUntil(6), RangeError);
});
