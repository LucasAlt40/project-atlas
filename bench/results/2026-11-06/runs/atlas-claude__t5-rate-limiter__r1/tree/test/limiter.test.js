const { test } = require('node:test');
const assert = require('node:assert');
const { createLimiter } = require('../src/limiter');

function fakeClock(start = 0) {
  const clock = { t: start };
  clock.now = () => clock.t;
  return clock;
}

test('rejects invalid capacity', () => {
  const now = fakeClock().now;
  for (const capacity of [undefined, 0, -1, 1.5, '3', NaN, Infinity, null]) {
    assert.throws(() => createLimiter({ capacity, refillPerSecond: 1, now }), RangeError, `capacity ${capacity}`);
  }
});

test('rejects invalid refillPerSecond', () => {
  const now = fakeClock().now;
  for (const refillPerSecond of [undefined, 0, -1, '1', NaN, Infinity, null]) {
    assert.throws(() => createLimiter({ capacity: 5, refillPerSecond, now }), RangeError, `refillPerSecond ${refillPerSecond}`);
  }
});

test('rejects a non-function now with TypeError', () => {
  assert.throws(() => createLimiter({ capacity: 5, refillPerSecond: 1, now: 123 }), TypeError);
});

test('accepts fractional refillPerSecond', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 0.5, now: clock.now });
  assert.strictEqual(limiter.available(), 5);
});

test('starts full', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.available(), 3);
});

test('tryRemove removes tokens when available and returns false without removing otherwise', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(2), true);
  assert.strictEqual(limiter.available(), 1);
  assert.strictEqual(limiter.tryRemove(2), false);
  assert.strictEqual(limiter.available(), 1, 'failed removal must not remove anything');
  assert.strictEqual(limiter.tryRemove(), true, 'n defaults to 1');
  assert.strictEqual(limiter.available(), 0);
});

test('refills continuously with elapsed time', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 2, now: clock.now });
  assert.strictEqual(limiter.tryRemove(10), true);
  clock.t += 250;
  assert.strictEqual(limiter.available(), 0, 'half a token is not yet available');
  clock.t += 250;
  assert.strictEqual(limiter.available(), 1);
  clock.t += 2000;
  assert.strictEqual(limiter.available(), 5);
});

test('refill never exceeds capacity', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 10, now: clock.now });
  assert.strictEqual(limiter.tryRemove(1), true);
  clock.t += 60000;
  assert.strictEqual(limiter.available(), 4);
});

test('available floors fractional tokens', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 0.5, now: clock.now });
  assert.strictEqual(limiter.tryRemove(5), true);
  clock.t += 1500;
  assert.strictEqual(limiter.available(), 0, '0.75 tokens floors to 0');
  clock.t += 500;
  assert.strictEqual(limiter.available(), 1, '1.0 token');
});

test('fractional refill rate adds tokens at the right pace', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 0.5, now: clock.now });
  assert.strictEqual(limiter.tryRemove(5), true);
  clock.t += 1000;
  assert.strictEqual(limiter.tryRemove(1), false, 'half a token after 1s');
  clock.t += 1000;
  assert.strictEqual(limiter.tryRemove(1), true, 'one token after 2s');
});

test('n must be a positive integer no greater than capacity', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  for (const n of [0, -1, 1.5, '2', NaN, 4]) {
    assert.throws(() => limiter.tryRemove(n), RangeError, `tryRemove(${n})`);
    assert.throws(() => limiter.msUntil(n), RangeError, `msUntil(${n})`);
  }
  assert.strictEqual(limiter.tryRemove(3), true, 'n equal to capacity is allowed');
});

test('msUntil returns 0 when tokens are already available', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.msUntil(3), 0);
});

test('msUntil rounds up and matches when tryRemove succeeds', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 3, now: clock.now });
  assert.strictEqual(limiter.tryRemove(5), true);
  // Need 2 tokens at 3/s: 2 / 3 s = 666.67 ms, rounded up to 667.
  const wait = limiter.msUntil(2);
  assert.strictEqual(wait, 667);
  clock.t += wait - 1;
  assert.strictEqual(limiter.tryRemove(2), false);
  clock.t += 1;
  assert.strictEqual(limiter.tryRemove(2), true);
});

test('msUntil with fractional rate 0.1 is exact at the returned time', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 0.1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.msUntil(), 10000);
  clock.t += 9999;
  assert.strictEqual(limiter.tryRemove(), false);
  clock.t += 1;
  assert.strictEqual(limiter.tryRemove(), true);
});

test('msUntil is never 0 while tokens are short, even at a high rate with a tiny shortfall', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 1e6, now: clock.now });
  assert.strictEqual(limiter.tryRemove(), true);
  // Leaves a shortfall of about 1e-7 tokens, which is below the ms precision EPS would cover.
  clock.t += 0.001 - 1e-10;
  assert.strictEqual(limiter.tryRemove(), false);
  const wait = limiter.msUntil();
  assert.ok(wait >= 1, `msUntil must be at least 1 while tokens are short, got ${wait}`);
  clock.t += wait;
  assert.strictEqual(limiter.tryRemove(), true);
});

test('default clock starts the bucket full', () => {
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1 });
  assert.strictEqual(limiter.available(), 2);
});

test('reset makes the bucket full again', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(4), true);
  limiter.reset();
  assert.strictEqual(limiter.available(), 4);
});

test('clock going backwards adds no tokens and loses none', () => {
  const clock = fakeClock(1000);
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(7), true);
  clock.t = 400;
  assert.strictEqual(limiter.available(), 3, 'no tokens lost when clock moves back');
  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.available(), 0, 'no tokens added when clock moves back');
});

test('earlier time becomes the new reference after clock regression', () => {
  const clock = fakeClock(1000);
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(10), true);
  clock.t = 500;
  assert.strictEqual(limiter.available(), 0);
  clock.t = 1500;
  // Refill is measured from 500, so one second later gives one token.
  assert.strictEqual(limiter.available(), 1);
});
