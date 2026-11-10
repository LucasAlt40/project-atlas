'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const { createLimiter } = require('../src/limiter');

// Returns a limiter driven by a fake clock that starts at 0 ms.
function fakeClock(options) {
  const clock = { t: 0 };
  const limiter = createLimiter({ ...options, now: () => clock.t });
  return { clock, limiter };
}

test('exports only createLimiter', () => {
  assert.deepStrictEqual(Object.keys(require('../src/limiter')), ['createLimiter']);
});

test('rejects invalid capacity', () => {
  for (const capacity of [undefined, 0, -1, 1.5, NaN, Infinity, '5']) {
    assert.throws(() => createLimiter({ capacity, refillPerSecond: 1 }), RangeError, `capacity ${capacity}`);
  }
});

test('rejects invalid refillPerSecond', () => {
  for (const refillPerSecond of [undefined, 0, -1, NaN, Infinity, '1']) {
    assert.throws(() => createLimiter({ capacity: 5, refillPerSecond }), RangeError, `refill ${refillPerSecond}`);
  }
});

test('accepts fractional refillPerSecond', () => {
  assert.doesNotThrow(() => createLimiter({ capacity: 5, refillPerSecond: 0.5 }));
});

test('starts full', () => {
  const { limiter } = fakeClock({ capacity: 5, refillPerSecond: 1 });
  assert.strictEqual(limiter.available(), 5);
});

test('tryRemove removes tokens when enough are available', () => {
  const { limiter } = fakeClock({ capacity: 5, refillPerSecond: 1 });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(4), true);
  assert.strictEqual(limiter.available(), 0);
});

test('tryRemove removes nothing when not enough tokens are available', () => {
  const { limiter } = fakeClock({ capacity: 5, refillPerSecond: 1 });
  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.tryRemove(3), false);
  assert.strictEqual(limiter.available(), 2);
});

test('tryRemove and msUntil reject invalid n', () => {
  const { limiter } = fakeClock({ capacity: 5, refillPerSecond: 1 });
  for (const n of [0, -1, 1.5, NaN, '1']) {
    assert.throws(() => limiter.tryRemove(n), RangeError, `tryRemove ${n}`);
    assert.throws(() => limiter.msUntil(n), RangeError, `msUntil ${n}`);
  }
});

test('tryRemove and msUntil reject n greater than capacity', () => {
  const { limiter } = fakeClock({ capacity: 5, refillPerSecond: 1 });
  assert.throws(() => limiter.tryRemove(6), RangeError);
  assert.throws(() => limiter.msUntil(6), RangeError);
});

test('refills continuously with elapsed time', () => {
  const { clock, limiter } = fakeClock({ capacity: 10, refillPerSecond: 2 });
  limiter.tryRemove(10);
  clock.t += 500;
  assert.strictEqual(limiter.available(), 1);
  clock.t += 250;
  assert.strictEqual(limiter.available(), 1);
  clock.t += 250;
  assert.strictEqual(limiter.available(), 2);
});

test('supports fractional refill rates', () => {
  const { clock, limiter } = fakeClock({ capacity: 5, refillPerSecond: 0.5 });
  limiter.tryRemove(5);
  clock.t += 1999;
  assert.strictEqual(limiter.available(), 0);
  clock.t += 1;
  assert.strictEqual(limiter.available(), 1);
});

test('never refills above capacity', () => {
  const { clock, limiter } = fakeClock({ capacity: 5, refillPerSecond: 1 });
  limiter.tryRemove(2);
  clock.t += 60000;
  assert.strictEqual(limiter.available(), 5);
});

test('msUntil is 0 when tokens are already available', () => {
  const { limiter } = fakeClock({ capacity: 5, refillPerSecond: 1 });
  assert.strictEqual(limiter.msUntil(5), 0);
});

test('msUntil rounds the wait up to whole milliseconds', () => {
  const { limiter } = fakeClock({ capacity: 10, refillPerSecond: 3 });
  limiter.tryRemove(10);
  assert.strictEqual(limiter.msUntil(1), 334);
  assert.strictEqual(limiter.msUntil(2), 667);
});

test('msUntil counts only the missing tokens', () => {
  const { clock, limiter } = fakeClock({ capacity: 10, refillPerSecond: 2 });
  limiter.tryRemove(10);
  clock.t += 1000;
  assert.strictEqual(limiter.msUntil(3), 500);
});

test('waiting msUntil(n) makes tryRemove(n) succeed, and one ms less does not', () => {
  for (const refillPerSecond of [0.1, 0.3, 3, 7]) {
    for (let n = 1; n <= 5; n++) {
      const { clock, limiter } = fakeClock({ capacity: 5, refillPerSecond });
      limiter.tryRemove(5);
      const wait = limiter.msUntil(n);
      assert.ok(wait > 0, `rate ${refillPerSecond} n ${n} should need to wait`);

      clock.t += wait - 1;
      assert.strictEqual(limiter.tryRemove(n), false, `rate ${refillPerSecond} n ${n} one ms early`);

      clock.t += 1;
      assert.strictEqual(limiter.tryRemove(n), true, `rate ${refillPerSecond} n ${n} at msUntil`);
    }
  }
});

test('reset makes the bucket full again', () => {
  const { limiter } = fakeClock({ capacity: 5, refillPerSecond: 1 });
  limiter.tryRemove(5);
  limiter.reset();
  assert.strictEqual(limiter.available(), 5);
});

test('clock going backwards adds no tokens and loses none', () => {
  const { clock, limiter } = fakeClock({ capacity: 10, refillPerSecond: 1 });
  clock.t = 1000;
  limiter.tryRemove(4);
  clock.t = 500;
  assert.strictEqual(limiter.available(), 6);
});

test('after clock goes backwards, refill resumes from the earlier time', () => {
  const { clock, limiter } = fakeClock({ capacity: 10, refillPerSecond: 1 });
  clock.t = 1000;
  limiter.tryRemove(10);
  clock.t = 500;
  assert.strictEqual(limiter.available(), 0);
  clock.t = 1500;
  assert.strictEqual(limiter.available(), 1);
});

test('uses Date.now when now is not given', () => {
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1 });
  assert.strictEqual(limiter.available(), 3);
  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.tryRemove(), false);
});
