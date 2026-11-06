'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const { createLimiter } = require('../src/limiter');

function clock(start = 0) {
  const c = { time: start };
  c.now = () => c.time;
  return c;
}

test('createLimiter rejects invalid capacity', () => {
  const bad = [undefined, 0, -1, 1.5, NaN, Infinity, '5', null];
  for (const capacity of bad) {
    assert.throws(() => createLimiter({ capacity, refillPerSecond: 1 }), RangeError);
  }
  assert.throws(() => createLimiter(), RangeError);
});

test('createLimiter rejects invalid refillPerSecond', () => {
  const bad = [undefined, 0, -1, NaN, Infinity, '1', null];
  for (const refillPerSecond of bad) {
    assert.throws(() => createLimiter({ capacity: 4, refillPerSecond }), RangeError);
  }
});

test('createLimiter accepts fractional refillPerSecond', () => {
  assert.doesNotThrow(() => createLimiter({ capacity: 2, refillPerSecond: 0.5, now: () => 0 }));
});

test('bucket starts full', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 2, now: c.now });
  assert.strictEqual(limiter.available(), 4);
});

test('tryRemove removes tokens and returns true while available', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(2), true);
  assert.strictEqual(limiter.available(), 0);
});

test('tryRemove returns false and removes nothing when not enough tokens', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.tryRemove(2), true);
  assert.strictEqual(limiter.tryRemove(2), false);
  assert.strictEqual(limiter.available(), 1);
});

test('tryRemove and msUntil reject invalid n', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 1, now: c.now });
  const bad = [0, -1, 1.5, NaN, '2', null];
  for (const n of bad) {
    assert.throws(() => limiter.tryRemove(n), RangeError);
    assert.throws(() => limiter.msUntil(n), RangeError);
  }
});

test('tryRemove and msUntil reject n greater than capacity', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 1, now: c.now });
  assert.throws(() => limiter.tryRemove(5), RangeError);
  assert.throws(() => limiter.msUntil(5), RangeError);
});

test('tokens refill continuously with elapsed time', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 2, now: c.now });
  assert.strictEqual(limiter.tryRemove(4), true);
  c.time += 500;
  assert.strictEqual(limiter.available(), 1);
  c.time += 250;
  assert.strictEqual(limiter.available(), 1);
  c.time += 250;
  assert.strictEqual(limiter.available(), 2);
});

test('refill never exceeds capacity', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 2, now: c.now });
  assert.strictEqual(limiter.tryRemove(1), true);
  c.time += 100000;
  assert.strictEqual(limiter.available(), 4);
});

test('fractional refill rate adds tokens slowly', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 0.5, now: c.now });
  assert.strictEqual(limiter.tryRemove(2), true);
  c.time += 1000;
  assert.strictEqual(limiter.available(), 0);
  assert.strictEqual(limiter.tryRemove(), false);
  c.time += 1000;
  assert.strictEqual(limiter.available(), 1);
  assert.strictEqual(limiter.tryRemove(), true);
});

test('msUntil returns 0 when tokens are already available', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 2, now: c.now });
  assert.strictEqual(limiter.msUntil(), 0);
  assert.strictEqual(limiter.msUntil(4), 0);
});

test('msUntil returns whole milliseconds until n tokens are available', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 2, now: c.now });
  assert.strictEqual(limiter.tryRemove(4), true);
  assert.strictEqual(limiter.msUntil(1), 500);
  assert.strictEqual(limiter.msUntil(3), 1500);
});

test('msUntil rounds up to a whole millisecond', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 3, now: c.now });
  assert.strictEqual(limiter.tryRemove(4), true);
  // 1000 / 3 = 333.33..., so the answer is rounded up to 334.
  assert.strictEqual(limiter.msUntil(1), 334);
});

test('msUntil accounts for partial refill', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 2, now: c.now });
  assert.strictEqual(limiter.tryRemove(4), true);
  c.time += 250;
  assert.strictEqual(limiter.msUntil(1), 250);
});

test('reset makes the bucket full again', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 1, now: c.now });
  assert.strictEqual(limiter.tryRemove(4), true);
  limiter.reset();
  assert.strictEqual(limiter.available(), 4);
});

test('reset re-reads now() as the new reference time', () => {
  const c = clock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 2, now: c.now });
  assert.strictEqual(limiter.tryRemove(4), true);
  c.time = 5000;
  limiter.reset();
  assert.strictEqual(limiter.tryRemove(4), true);
  c.time += 500;
  assert.strictEqual(limiter.available(), 1);
});

test('now defaults to Date.now when not provided', () => {
  const realNow = Date.now;
  let time = 1000;
  Date.now = () => time;
  try {
    const limiter = createLimiter({ capacity: 4, refillPerSecond: 2 });
    assert.strictEqual(limiter.tryRemove(4), true);
    time += 500;
    assert.strictEqual(limiter.available(), 1);
  } finally {
    Date.now = realNow;
  }
});

test('clock going backwards adds no tokens and loses none', () => {
  const c = clock(1000);
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 2, now: c.now });
  assert.strictEqual(limiter.tryRemove(4), true);
  c.time = 1250;
  assert.strictEqual(limiter.available(), 0); // 0.5 tokens, floored
  c.time = 1000; // clock jumps back
  assert.strictEqual(limiter.available(), 0); // no tokens added
  c.time = 1250; // 250 ms forward from the earlier reference
  assert.strictEqual(limiter.available(), 1); // 0.5 kept + 0.5 added
});
