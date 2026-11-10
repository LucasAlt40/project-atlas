'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const limiter = require('../src/limiter');
const { createLimiter } = limiter;

// Returns a clock whose time is set and advanced by the test.
function fakeClock(start = 0) {
  let time = start;
  const now = () => time;
  now.set = (value) => {
    time = value;
  };
  now.advance = (ms) => {
    time += ms;
  };
  return now;
}

test('exports only createLimiter', () => {
  assert.deepStrictEqual(Object.keys(limiter), ['createLimiter']);
});

test('starts full', () => {
  const bucket = createLimiter({ capacity: 5, refillPerSecond: 1, now: fakeClock() });
  assert.strictEqual(bucket.available(), 5);
});

test('tryRemove removes tokens when enough are available', () => {
  const bucket = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock() });
  assert.strictEqual(bucket.tryRemove(), true);
  assert.strictEqual(bucket.tryRemove(2), true);
  assert.strictEqual(bucket.available(), 0);
});

test('tryRemove returns false and removes nothing when not enough tokens', () => {
  const bucket = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock() });
  assert.strictEqual(bucket.tryRemove(2), true);
  assert.strictEqual(bucket.tryRemove(2), false);
  assert.strictEqual(bucket.available(), 1);
});

test('tryRemove with n equal to capacity works when the bucket is full', () => {
  const bucket = createLimiter({ capacity: 4, refillPerSecond: 1, now: fakeClock() });
  assert.strictEqual(bucket.tryRemove(4), true);
  assert.strictEqual(bucket.available(), 0);
});

test('tokens refill over time', () => {
  const now = fakeClock();
  const bucket = createLimiter({ capacity: 5, refillPerSecond: 1, now });
  assert.strictEqual(bucket.tryRemove(5), true);
  now.advance(2000);
  assert.strictEqual(bucket.available(), 2);
});

test('refill never exceeds capacity', () => {
  const now = fakeClock();
  const bucket = createLimiter({ capacity: 5, refillPerSecond: 10, now });
  assert.strictEqual(bucket.tryRemove(1), true);
  now.advance(60000);
  assert.strictEqual(bucket.available(), 5);
});

test('fractional refill rate accumulates fractional tokens', () => {
  const now = fakeClock();
  const bucket = createLimiter({ capacity: 3, refillPerSecond: 0.5, now });
  assert.strictEqual(bucket.tryRemove(3), true);
  now.advance(1000);
  assert.strictEqual(bucket.available(), 0);
  assert.strictEqual(bucket.tryRemove(), false);
  now.advance(1000);
  assert.strictEqual(bucket.available(), 1);
});

test('available floors the fractional token count', () => {
  const now = fakeClock();
  const bucket = createLimiter({ capacity: 5, refillPerSecond: 1, now });
  assert.strictEqual(bucket.tryRemove(5), true);
  now.advance(2750);
  assert.strictEqual(bucket.available(), 2);
});

test('float drift does not reject a request that should succeed', () => {
  const now = fakeClock();
  const bucket = createLimiter({ capacity: 5, refillPerSecond: 1, now });
  assert.strictEqual(bucket.tryRemove(5), true);
  // Ten steps of 0.1 tokens each; the sum may drift just below 1.
  for (let i = 0; i < 10; i += 1) {
    now.advance(100);
    bucket.available();
  }
  assert.strictEqual(bucket.available(), 1);
  assert.strictEqual(bucket.tryRemove(1), true);
});

test('msUntil returns 0 when the tokens are already available', () => {
  const bucket = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock() });
  assert.strictEqual(bucket.msUntil(), 0);
  assert.strictEqual(bucket.msUntil(3), 0);
});

test('msUntil rounds the wait up to whole milliseconds', () => {
  const now = fakeClock();
  const bucket = createLimiter({ capacity: 5, refillPerSecond: 3, now });
  assert.strictEqual(bucket.tryRemove(5), true);
  // One token takes 1000 / 3 = 333.33... ms, so it rounds up to 334.
  assert.strictEqual(bucket.msUntil(), 334);
});

test('msUntil accounts for partially refilled tokens', () => {
  const now = fakeClock();
  const bucket = createLimiter({ capacity: 5, refillPerSecond: 1, now });
  assert.strictEqual(bucket.tryRemove(5), true);
  now.advance(250);
  // 0.25 tokens exist, so 0.75 more are needed: 750 ms.
  assert.strictEqual(bucket.msUntil(), 750);
  assert.strictEqual(bucket.msUntil(2), 1750);
});

test('tryRemove succeeds once the time reported by msUntil has passed', () => {
  const now = fakeClock();
  const bucket = createLimiter({ capacity: 4, refillPerSecond: 0.7, now });
  assert.strictEqual(bucket.tryRemove(4), true);
  const wait = bucket.msUntil(2);
  assert.strictEqual(bucket.tryRemove(2), false);
  now.advance(wait);
  assert.strictEqual(bucket.tryRemove(2), true);
});

test('invalid n throws RangeError and leaves the bucket unchanged', () => {
  const bucket = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock() });
  for (const n of [0, -1, 1.5, NaN, Infinity, '1', null, 4]) {
    assert.throws(() => bucket.tryRemove(n), RangeError);
    assert.throws(() => bucket.msUntil(n), RangeError);
  }
  assert.strictEqual(bucket.available(), 3);
});

test('reset makes the bucket full again', () => {
  const bucket = createLimiter({ capacity: 4, refillPerSecond: 1, now: fakeClock() });
  assert.strictEqual(bucket.tryRemove(4), true);
  bucket.reset();
  assert.strictEqual(bucket.available(), 4);
});

test('reset sets the reference time to now', () => {
  const now = fakeClock();
  const bucket = createLimiter({ capacity: 3, refillPerSecond: 1, now });
  assert.strictEqual(bucket.tryRemove(3), true);
  now.advance(500);
  bucket.reset();
  assert.strictEqual(bucket.tryRemove(3), true);
  now.advance(500);
  // Only 0.5 tokens have refilled since the reset.
  assert.strictEqual(bucket.available(), 0);
  now.advance(500);
  assert.strictEqual(bucket.available(), 1);
});

test('clock going backwards adds and loses no tokens', () => {
  const now = fakeClock(10000);
  const bucket = createLimiter({ capacity: 10, refillPerSecond: 1, now });
  assert.strictEqual(bucket.tryRemove(10), true);
  now.set(4000);
  assert.strictEqual(bucket.available(), 0);
  // The earlier time is the new reference, so 1 second later gives 1 token.
  now.set(5000);
  assert.strictEqual(bucket.available(), 1);
});

test('createLimiter throws RangeError for an invalid capacity', () => {
  for (const capacity of [undefined, 0, -1, 1.5, NaN, Infinity, '5']) {
    assert.throws(() => createLimiter({ capacity, refillPerSecond: 1 }), RangeError);
  }
});

test('createLimiter throws RangeError for an invalid refillPerSecond', () => {
  for (const refillPerSecond of [undefined, 0, -1, NaN, Infinity, '1']) {
    assert.throws(() => createLimiter({ capacity: 1, refillPerSecond }), RangeError);
  }
});

test('createLimiter throws TypeError when now is not a function', () => {
  assert.throws(
    () => createLimiter({ capacity: 1, refillPerSecond: 1, now: 123 }),
    TypeError,
  );
});

test('createLimiter defaults now to Date.now', () => {
  // A very slow refill keeps the real clock from adding a token during the test.
  const bucket = createLimiter({ capacity: 2, refillPerSecond: 0.001 });
  assert.strictEqual(bucket.tryRemove(2), true);
  assert.strictEqual(bucket.tryRemove(), false);
  assert.ok(bucket.msUntil() > 0);
});
