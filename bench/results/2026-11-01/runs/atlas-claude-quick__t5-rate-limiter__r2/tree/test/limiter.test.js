'use strict';

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

test('createLimiter rejects missing or invalid options with RangeError', () => {
  assert.throws(() => createLimiter(), RangeError);
  assert.throws(() => createLimiter({}), RangeError);

  for (const capacity of [0, -1, 1.5, '5', NaN, Infinity, undefined]) {
    assert.throws(
      () => createLimiter({ capacity, refillPerSecond: 1 }),
      RangeError,
      `capacity ${capacity}`
    );
  }

  for (const refillPerSecond of [0, -1, NaN, Infinity, '1', undefined]) {
    assert.throws(
      () => createLimiter({ capacity: 1, refillPerSecond }),
      RangeError,
      `refillPerSecond ${refillPerSecond}`
    );
  }
});

test('accepts a fractional refillPerSecond', () => {
  const clock = fakeClock();
  assert.doesNotThrow(() =>
    createLimiter({ capacity: 1, refillPerSecond: 0.5, now: clock.now })
  );
});

test('starts full and reset makes it full again', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.available(), 3);

  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.available(), 0);

  limiter.reset();
  assert.strictEqual(limiter.available(), 3);
});

test('tryRemove removes n tokens or nothing', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });

  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.tryRemove(3), false);
  assert.strictEqual(limiter.available(), 2, 'failed request removes nothing');

  assert.strictEqual(limiter.tryRemove(), true, 'n defaults to 1');
  assert.strictEqual(limiter.available(), 1);
});

test('tryRemove and msUntil reject invalid n with RangeError', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });

  for (const n of [0, -1, 1.5, '2', NaN, Infinity, null]) {
    assert.throws(() => limiter.tryRemove(n), RangeError, `tryRemove(${n})`);
    assert.throws(() => limiter.msUntil(n), RangeError, `msUntil(${n})`);
  }
});

test('tryRemove and msUntil reject n greater than capacity', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });

  assert.throws(() => limiter.tryRemove(6), RangeError);
  assert.throws(() => limiter.msUntil(6), RangeError);
  assert.strictEqual(limiter.tryRemove(5), true, 'n equal to capacity is allowed');
});

test('refills continuously and never above capacity', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 2, now: clock.now });

  assert.strictEqual(limiter.tryRemove(10), true);
  assert.strictEqual(limiter.available(), 0);

  clock.advance(500);
  assert.strictEqual(limiter.available(), 1);

  clock.advance(500);
  assert.strictEqual(limiter.available(), 2);

  clock.advance(1000000);
  assert.strictEqual(limiter.available(), 10);
});

test('available floors fractional tokens', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });

  assert.strictEqual(limiter.tryRemove(5), true);
  clock.advance(1500);
  assert.strictEqual(limiter.available(), 1);
});

test('fractional refill rate accumulates across calls', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 0.5, now: clock.now });

  assert.strictEqual(limiter.tryRemove(2), true);
  clock.advance(1000);
  assert.strictEqual(limiter.tryRemove(), false, 'only 0.5 tokens after 1s');
  clock.advance(1000);
  assert.strictEqual(limiter.tryRemove(), true, '1 token after 2s');
});

test('msUntil is 0 when tokens are already available', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 2, now: clock.now });

  assert.strictEqual(limiter.msUntil(), 0);
  assert.strictEqual(limiter.msUntil(4), 0);
});

test('msUntil gives the whole milliseconds until n tokens exist', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 2, now: clock.now });

  assert.strictEqual(limiter.tryRemove(4), true);
  assert.strictEqual(limiter.msUntil(1), 500);
  assert.strictEqual(limiter.msUntil(2), 1000);

  clock.advance(250);
  assert.strictEqual(limiter.msUntil(1), 250);
});

test('msUntil rounds up partial milliseconds', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 3, now: clock.now });

  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.msUntil(), 334, 'ceil(1000 / 3)');
});

test('msUntil accounts for fractional tokens already present', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 0.5, now: clock.now });

  assert.strictEqual(limiter.tryRemove(2), true);
  clock.advance(500);
  assert.strictEqual(limiter.msUntil(1), 1500, '0.25 tokens present, need 0.75 more');
});

test('a backwards clock adds no tokens and loses none', () => {
  const clock = fakeClock(1000);
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 1, now: clock.now });

  assert.strictEqual(limiter.tryRemove(10), true);
  clock.set(1500);
  assert.strictEqual(limiter.available(), 0, 'half a token is pending');

  clock.set(1000);
  assert.strictEqual(limiter.available(), 0, 'no tokens added on backwards step');

  clock.set(1500);
  assert.strictEqual(limiter.available(), 1, 'the pending half token was kept');
});

test('the earlier time becomes the new reference after the clock goes backwards', () => {
  const clock = fakeClock(1000);
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 1, now: clock.now });

  assert.strictEqual(limiter.tryRemove(10), true);
  clock.set(2500);
  assert.strictEqual(limiter.available(), 1, '1.5 tokens');

  clock.set(1000);
  assert.strictEqual(limiter.available(), 1, 'no tokens added or lost');

  clock.set(1500);
  assert.strictEqual(limiter.available(), 2, 'counted from 1000: 1.5 + 0.5 tokens');
});

test('reset makes the bucket full and resets the time reference', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });

  assert.strictEqual(limiter.tryRemove(3), true);
  clock.advance(10000);
  limiter.reset();
  assert.strictEqual(limiter.available(), 3);
  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.msUntil(), 1000);
});
