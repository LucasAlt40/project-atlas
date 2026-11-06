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
    set: (time) => {
      t = time;
    },
  };
}

test('exports only createLimiter', () => {
  assert.deepStrictEqual(Object.keys(require('../src/limiter')), ['createLimiter']);
});

test('bucket starts full and empties after capacity removals', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });

  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), false);
});

test('tryRemove defaults to one token', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });

  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.available(), 4);
});

test('a failed removal takes nothing', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });

  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.tryRemove(3), false);
  assert.strictEqual(limiter.available(), 2);
});

test('refill is continuous with elapsed time', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 2, now: clock.now });

  assert.strictEqual(limiter.tryRemove(10), true);
  clock.advance(499);
  assert.strictEqual(limiter.available(), 0);
  clock.advance(1);
  assert.strictEqual(limiter.available(), 1);
});

test('fractional refill rate', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 0.5, now: clock.now });

  assert.strictEqual(limiter.tryRemove(2), true);
  clock.advance(1999);
  assert.strictEqual(limiter.tryRemove(), false);
  clock.advance(1);
  assert.strictEqual(limiter.tryRemove(), true);
});

test('tokens never exceed capacity', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });

  assert.strictEqual(limiter.tryRemove(), true);
  clock.advance(1_000_000);
  assert.strictEqual(limiter.available(), 3);
});

test('available rounds down', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 1, now: clock.now });

  assert.strictEqual(limiter.tryRemove(10), true);
  clock.advance(2500);
  assert.strictEqual(limiter.available(), 2);
});

test('msUntil is 0 when tokens are already available', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });

  assert.strictEqual(limiter.msUntil(), 0);
  assert.strictEqual(limiter.msUntil(3), 0);
});

test('msUntil rounds the wait up to whole milliseconds', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 3, now: clock.now });

  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.msUntil(), 334); // 1000 / 3 = 333.33...
  assert.strictEqual(limiter.msUntil(2), 667); // 2000 / 3 = 666.67...

  clock.advance(100); // 0.3 tokens earned
  assert.strictEqual(limiter.msUntil(), 234); // 0.7 tokens still needed
});

test('msUntil(n) is exact: waiting that long makes tryRemove(n) succeed', () => {
  for (const refillPerSecond of [0.1, 0.3, 0.7, 3, 1 / 3]) {
    for (let n = 1; n <= 5; n++) {
      const clock = fakeClock();
      const limiter = createLimiter({ capacity: 5, refillPerSecond, now: clock.now });

      assert.strictEqual(limiter.tryRemove(5), true);
      const wait = limiter.msUntil(n);
      assert.ok(wait > 0, `wait for n=${n} at ${refillPerSecond}/s`);

      clock.advance(wait - 1);
      assert.strictEqual(limiter.tryRemove(n), false, `too early for n=${n} at ${refillPerSecond}/s`);
      clock.advance(1);
      assert.strictEqual(limiter.tryRemove(n), true, `on time for n=${n} at ${refillPerSecond}/s`);
    }
  }
});

test('reset makes the bucket full again', () => {
  const clock = fakeClock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 1, now: clock.now });

  assert.strictEqual(limiter.tryRemove(4), true);
  assert.strictEqual(limiter.available(), 0);
  limiter.reset();
  assert.strictEqual(limiter.available(), 4);
});

test('clock going backwards adds no tokens and loses none', () => {
  const clock = fakeClock(0);
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 1, now: clock.now });

  assert.strictEqual(limiter.tryRemove(5), true);
  clock.set(3000);
  assert.strictEqual(limiter.available(), 8);

  clock.set(1000); // backwards: no tokens added, none lost
  assert.strictEqual(limiter.available(), 8);

  clock.set(2000); // one second after the new reference
  assert.strictEqual(limiter.available(), 9);
});

test('clock going backwards does not refill an empty bucket', () => {
  const clock = fakeClock(10_000);
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 1, now: clock.now });

  assert.strictEqual(limiter.tryRemove(10), true);
  clock.set(4000);
  assert.strictEqual(limiter.available(), 0);
  clock.set(5000);
  assert.strictEqual(limiter.available(), 1);
});

test('createLimiter rejects invalid capacity', () => {
  const bad = [undefined, 0, -1, 1.5, NaN, Infinity, '3', null];
  for (const capacity of bad) {
    assert.throws(() => createLimiter({ capacity, refillPerSecond: 1 }), RangeError, `capacity ${capacity}`);
  }
});

test('createLimiter rejects invalid refillPerSecond', () => {
  const bad = [undefined, 0, -1, NaN, Infinity, -Infinity, '1', null];
  for (const refillPerSecond of bad) {
    assert.throws(() => createLimiter({ capacity: 1, refillPerSecond }), RangeError, `refill ${refillPerSecond}`);
  }
});

test('createLimiter rejects missing options', () => {
  assert.throws(() => createLimiter(), RangeError);
});

test('invalid n is rejected by tryRemove and msUntil', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1 });
  const bad = [0, -1, 1.5, NaN, '2', null];
  for (const n of bad) {
    assert.throws(() => limiter.tryRemove(n), RangeError, `tryRemove(${n})`);
    assert.throws(() => limiter.msUntil(n), RangeError, `msUntil(${n})`);
  }
});

test('n above capacity can never succeed and throws', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1 });
  assert.throws(() => limiter.tryRemove(4), RangeError);
  assert.throws(() => limiter.msUntil(4), RangeError);
});

test('defaults to Date.now when no clock is given', () => {
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1 });
  assert.strictEqual(limiter.tryRemove(2), true);
  assert.strictEqual(limiter.tryRemove(), false);
});
