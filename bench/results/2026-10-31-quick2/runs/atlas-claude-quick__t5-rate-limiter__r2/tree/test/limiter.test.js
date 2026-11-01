'use strict';

const { test, describe } = require('node:test');
const assert = require('node:assert');
const { createLimiter } = require('../src/limiter');

// Returns a limiter driven by a manually advanced clock.
function setup(options) {
  const clock = { t: 0 };
  const limiter = createLimiter({ ...options, now: () => clock.t });
  return { clock, limiter };
}

describe('createLimiter options', () => {
  const invalidCapacities = [undefined, 0, -1, 1.5, '5', NaN, Infinity, null];
  for (const capacity of invalidCapacities) {
    test(`rejects capacity ${String(capacity)}`, () => {
      assert.throws(() => createLimiter({ capacity, refillPerSecond: 1 }), RangeError);
    });
  }

  const invalidRates = [undefined, 0, -1, '1', NaN, Infinity];
  for (const refillPerSecond of invalidRates) {
    test(`rejects refillPerSecond ${String(refillPerSecond)}`, () => {
      assert.throws(() => createLimiter({ capacity: 1, refillPerSecond }), RangeError);
    });
  }

  test('rejects a missing options object', () => {
    assert.throws(() => createLimiter(), RangeError);
    assert.throws(() => createLimiter(null), RangeError);
  });

  test('accepts a fractional refillPerSecond', () => {
    assert.doesNotThrow(() => createLimiter({ capacity: 1, refillPerSecond: 0.5 }));
  });
});

describe('tryRemove', () => {
  test('starts full and removes tokens until empty', () => {
    const { limiter } = setup({ capacity: 2, refillPerSecond: 1 });
    assert.strictEqual(limiter.tryRemove(), true);
    assert.strictEqual(limiter.tryRemove(), true);
    assert.strictEqual(limiter.tryRemove(), false);
  });

  test('removes n tokens at once, or nothing when there are not enough', () => {
    const { limiter } = setup({ capacity: 5, refillPerSecond: 1 });
    assert.strictEqual(limiter.tryRemove(3), true);
    assert.strictEqual(limiter.tryRemove(3), false);
    assert.strictEqual(limiter.available(), 2);
  });

  test('rejects n that is not a positive integer', () => {
    const { limiter } = setup({ capacity: 5, refillPerSecond: 1 });
    for (const n of [0, -1, 1.5, NaN, '1']) {
      assert.throws(() => limiter.tryRemove(n), RangeError);
    }
  });

  test('rejects n greater than capacity', () => {
    const { limiter } = setup({ capacity: 5, refillPerSecond: 1 });
    assert.throws(() => limiter.tryRemove(6), RangeError);
  });
});

describe('refill', () => {
  test('adds tokens continuously with elapsed time', () => {
    const { clock, limiter } = setup({ capacity: 10, refillPerSecond: 2 });
    limiter.tryRemove(10);
    clock.t = 500;
    assert.strictEqual(limiter.available(), 1);
    clock.t = 1000;
    assert.strictEqual(limiter.available(), 2);
  });

  test('handles fractional refill rates', () => {
    const { clock, limiter } = setup({ capacity: 3, refillPerSecond: 0.5 });
    limiter.tryRemove(3);
    clock.t = 1000;
    assert.strictEqual(limiter.available(), 0);
    clock.t = 2000;
    assert.strictEqual(limiter.available(), 1);
  });

  test('never grows above capacity', () => {
    const { clock, limiter } = setup({ capacity: 3, refillPerSecond: 1 });
    limiter.tryRemove(1);
    clock.t = 1_000_000;
    assert.strictEqual(limiter.available(), 3);
  });

  test('available rounds the token count down', () => {
    const { clock, limiter } = setup({ capacity: 5, refillPerSecond: 1 });
    limiter.tryRemove(5);
    clock.t = 1900;
    assert.strictEqual(limiter.available(), 1);
  });
});

describe('msUntil', () => {
  test('returns 0 when the tokens are already available', () => {
    const { limiter } = setup({ capacity: 2, refillPerSecond: 1 });
    assert.strictEqual(limiter.msUntil(2), 0);
  });

  test('returns the time until n tokens are available', () => {
    const { limiter } = setup({ capacity: 5, refillPerSecond: 2 });
    limiter.tryRemove(5);
    assert.strictEqual(limiter.msUntil(1), 500);
    assert.strictEqual(limiter.msUntil(2), 1000);
  });

  test('rounds the wait up to a whole millisecond', () => {
    const { limiter } = setup({ capacity: 1, refillPerSecond: 3 });
    limiter.tryRemove(1);
    assert.strictEqual(limiter.msUntil(1), 334);
  });

  test('is 0 exactly when tryRemove would succeed', () => {
    const { clock, limiter } = setup({ capacity: 1, refillPerSecond: 3 });
    limiter.tryRemove(1);
    const wait = limiter.msUntil(1);
    clock.t = wait - 1;
    assert.ok(limiter.msUntil(1) > 0);
    assert.strictEqual(limiter.tryRemove(1), false);
    clock.t = wait;
    assert.strictEqual(limiter.msUntil(1), 0);
    assert.strictEqual(limiter.tryRemove(1), true);
  });

  test('rejects invalid n', () => {
    const { limiter } = setup({ capacity: 2, refillPerSecond: 1 });
    assert.throws(() => limiter.msUntil(0), RangeError);
    assert.throws(() => limiter.msUntil(2.5), RangeError);
    assert.throws(() => limiter.msUntil(3), RangeError);
  });
});

describe('reset', () => {
  test('makes the bucket full again', () => {
    const { limiter } = setup({ capacity: 4, refillPerSecond: 1 });
    limiter.tryRemove(4);
    limiter.reset();
    assert.strictEqual(limiter.available(), 4);
  });
});

describe('clock regression', () => {
  test('adds no tokens and loses none when the clock goes backwards', () => {
    const { clock, limiter } = setup({ capacity: 5, refillPerSecond: 1 });
    clock.t = 1000;
    limiter.tryRemove(3);
    clock.t = 900;
    assert.strictEqual(limiter.available(), 2);
  });

  test('uses the earlier time as the new reference', () => {
    const { clock, limiter } = setup({ capacity: 5, refillPerSecond: 1 });
    limiter.tryRemove(5);
    clock.t = 2000;
    assert.strictEqual(limiter.available(), 2);
    clock.t = 1000;
    assert.strictEqual(limiter.available(), 2);
    clock.t = 1500;
    assert.strictEqual(limiter.available(), 2);
    clock.t = 2000;
    assert.strictEqual(limiter.available(), 3);
  });
});
