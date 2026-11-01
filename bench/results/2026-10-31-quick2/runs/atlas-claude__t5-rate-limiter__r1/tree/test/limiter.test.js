const { test } = require('node:test');
const assert = require('node:assert');
const { createLimiter } = require('../src/limiter');

// A clock the test controls, so refill timing is exact.
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

test('throws RangeError for an invalid capacity', () => {
  const bad = [undefined, 0, -1, 1.5, NaN, Infinity, '5', null];
  for (const capacity of bad) {
    assert.throws(() => createLimiter({ capacity, refillPerSecond: 1 }), RangeError);
  }
});

test('throws RangeError for an invalid refillPerSecond', () => {
  const bad = [undefined, 0, -1, NaN, Infinity, '1', null];
  for (const refillPerSecond of bad) {
    assert.throws(() => createLimiter({ capacity: 5, refillPerSecond }), RangeError);
  }
});

test('throws RangeError for an invalid n', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
  const bad = [0, -1, 1.5, NaN, '1', 4];
  for (const n of bad) {
    assert.throws(() => limiter.tryRemove(n), RangeError);
    assert.throws(() => limiter.msUntil(n), RangeError);
  }
});

test('an invalid n leaves the bucket unchanged', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  assert.throws(() => limiter.tryRemove(4), RangeError);
  assert.equal(limiter.available(), 3);
});

test('starts full', () => {
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: fakeClock().now });
  assert.equal(limiter.available(), 5);
});

test('tryRemove removes tokens when enough are available', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
  assert.equal(limiter.tryRemove(), true);
  assert.equal(limiter.tryRemove(2), true);
  assert.equal(limiter.available(), 0);
});

test('tryRemove returns false and removes nothing when too few tokens are available', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
  assert.equal(limiter.tryRemove(2), true);
  assert.equal(limiter.tryRemove(2), false);
  assert.equal(limiter.available(), 1);
});

test('tokens refill over time at refillPerSecond', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 2, now: clock.now });
  limiter.tryRemove(10);
  clock.advance(1000);
  assert.equal(limiter.available(), 2);
});

test('fractional refill accumulates before it becomes a whole token', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 0.5, now: clock.now });
  limiter.tryRemove(5);
  clock.advance(1000);
  assert.equal(limiter.available(), 0);
  clock.advance(1000);
  assert.equal(limiter.available(), 1);
});

test('tokens never grow above capacity', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(1);
  clock.advance(60000);
  assert.equal(limiter.available(), 3);
});

test('available rounds down to a whole number', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(5);
  clock.advance(1500);
  assert.equal(limiter.available(), 1);
});

test('msUntil is 0 when the tokens are already available', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
  assert.equal(limiter.msUntil(3), 0);
});

test('msUntil rounds the wait up to whole milliseconds', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 3, now: clock.now });
  limiter.tryRemove(5);
  // One token at 3/s takes 1000/3 ms = 333.33... ms, which rounds up to 334.
  assert.equal(limiter.msUntil(1), 334);
});

test('msUntil counts the wait for several tokens', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 0.5, now: clock.now });
  limiter.tryRemove(5);
  // Two tokens at 0.5/s take 4000 ms.
  assert.equal(limiter.msUntil(2), 4000);
  clock.advance(1000);
  assert.equal(limiter.msUntil(2), 3000);
});

test('msUntil matches when tryRemove starts to succeed', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 3, now: clock.now });
  limiter.tryRemove(5);
  const wait = limiter.msUntil(1);
  clock.advance(wait - 1);
  assert.equal(limiter.tryRemove(), false);
  clock.advance(1);
  assert.equal(limiter.tryRemove(), true);
});

test('reset makes the bucket full again', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(4);
  limiter.reset();
  assert.equal(limiter.available(), 4);
});

test('a clock that goes backwards adds no tokens and loses none', () => {
  const clock = fakeClock(10000);
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(10);
  clock.set(5000);
  assert.equal(limiter.available(), 0);
  // The earlier time is the new reference, so 1 s later gives 1 token.
  clock.advance(1000);
  assert.equal(limiter.available(), 1);
});

test('a clock that goes backwards keeps tokens that were already there', () => {
  const clock = fakeClock(10000);
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(4);
  clock.set(2000);
  assert.equal(limiter.available(), 6);
});

test('fractional refill does not lose a whole token to floating-point drift', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 0.1, now: clock.now });
  limiter.tryRemove(10);
  // 100 steps of 100 ms at 0.1/s is exactly 1 token, despite float rounding.
  for (let i = 0; i < 100; i++) {
    clock.advance(100);
    limiter.available();
  }
  assert.equal(limiter.available(), 1);
  assert.equal(limiter.tryRemove(), true);
  assert.equal(limiter.available(), 0);
});

test('works with the default clock when no now is given', () => {
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1 });
  assert.equal(limiter.tryRemove(2), true);
  assert.equal(limiter.tryRemove(), false);
});
