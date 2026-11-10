'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const { createLimiter } = require('../src/limiter');

// Returns a limiter driven by a manually advanced clock.
function setup(options) {
  const clock = { t: 0 };
  const limiter = createLimiter({ ...options, now: () => clock.t });
  return { clock, limiter };
}

test('rejects invalid capacity', () => {
  const bad = [undefined, 0, -1, 1.5, NaN, Infinity, '5', null];
  for (const capacity of bad) {
    assert.throws(() => createLimiter({ capacity, refillPerSecond: 1 }), RangeError, `capacity=${capacity}`);
  }
});

test('rejects invalid refillPerSecond', () => {
  const bad = [undefined, 0, -1, NaN, Infinity, '1', null];
  for (const refillPerSecond of bad) {
    assert.throws(() => createLimiter({ capacity: 1, refillPerSecond }), RangeError, `refill=${refillPerSecond}`);
  }
});

test('throws RangeError when options are missing', () => {
  assert.throws(() => createLimiter(), RangeError);
});

test('accepts fractional refillPerSecond', () => {
  assert.doesNotThrow(() => createLimiter({ capacity: 1, refillPerSecond: 0.5 }));
});

test('uses Date.now when now is not given', () => {
  const limiter = createLimiter({ capacity: 2, refillPerSecond: 1 });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.available(), 1);
});

test('starts full', () => {
  const { limiter } = setup({ capacity: 3, refillPerSecond: 1 });
  assert.strictEqual(limiter.available(), 3);
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), false);
});

test('tryRemove(n) removes n tokens when enough are available', () => {
  const { limiter } = setup({ capacity: 5, refillPerSecond: 1 });
  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.available(), 2);
});

test('tryRemove(n) removes nothing when not enough are available', () => {
  const { limiter } = setup({ capacity: 5, refillPerSecond: 1 });
  assert.strictEqual(limiter.tryRemove(4), true);
  assert.strictEqual(limiter.tryRemove(2), false);
  assert.strictEqual(limiter.available(), 1);
});

test('refills continuously with elapsed time', () => {
  const { clock, limiter } = setup({ capacity: 10, refillPerSecond: 2 });
  assert.strictEqual(limiter.tryRemove(10), true);
  assert.strictEqual(limiter.available(), 0);
  clock.t += 500;
  assert.strictEqual(limiter.available(), 1);
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.tryRemove(), false);
});

test('never refills above capacity', () => {
  const { clock, limiter } = setup({ capacity: 4, refillPerSecond: 1 });
  assert.strictEqual(limiter.tryRemove(), true);
  clock.t += 60 * 60 * 1000;
  assert.strictEqual(limiter.available(), 4);
});

test('supports fractional refill rates', () => {
  const { clock, limiter } = setup({ capacity: 2, refillPerSecond: 0.5 });
  assert.strictEqual(limiter.tryRemove(2), true);
  clock.t += 1000;
  assert.strictEqual(limiter.available(), 0);
  clock.t += 1000;
  assert.strictEqual(limiter.available(), 1);
});

test('accumulated fractional refills reach whole tokens without drift', () => {
  const { clock, limiter } = setup({ capacity: 1, refillPerSecond: 0.1 });
  assert.strictEqual(limiter.tryRemove(), true);
  for (let i = 0; i < 100; i++) {
    clock.t += 100;
    limiter.available();
  }
  assert.strictEqual(limiter.available(), 1);
  assert.strictEqual(limiter.tryRemove(), true);
});

test('available() rounds down', () => {
  const { clock, limiter } = setup({ capacity: 5, refillPerSecond: 1 });
  assert.strictEqual(limiter.tryRemove(5), true);
  clock.t += 2999;
  assert.strictEqual(limiter.available(), 2);
});

test('msUntil returns 0 when tokens are already available', () => {
  const { limiter } = setup({ capacity: 3, refillPerSecond: 1 });
  assert.strictEqual(limiter.msUntil(), 0);
  assert.strictEqual(limiter.msUntil(3), 0);
});

test('msUntil returns the time until n tokens are available', () => {
  const { limiter } = setup({ capacity: 4, refillPerSecond: 2 });
  assert.strictEqual(limiter.tryRemove(4), true);
  assert.strictEqual(limiter.msUntil(), 500);
  assert.strictEqual(limiter.msUntil(3), 1500);
});

test('msUntil accounts for tokens already refilled', () => {
  const { clock, limiter } = setup({ capacity: 4, refillPerSecond: 2 });
  assert.strictEqual(limiter.tryRemove(4), true);
  clock.t += 250;
  assert.strictEqual(limiter.msUntil(), 250);
});

test('msUntil works with fractional refill rates', () => {
  const { limiter } = setup({ capacity: 1, refillPerSecond: 0.5 });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.msUntil(), 2000);
});

test('msUntil rounds up to whole milliseconds', () => {
  const { limiter } = setup({ capacity: 1, refillPerSecond: 3 });
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.msUntil(), 334);
});

test('msUntil agrees with tryRemove after waiting that long', () => {
  const { clock, limiter } = setup({ capacity: 5, refillPerSecond: 3 });
  assert.strictEqual(limiter.tryRemove(5), true);
  clock.t += limiter.msUntil(2);
  assert.strictEqual(limiter.tryRemove(2), true);
});

test('tryRemove, msUntil and available reject invalid n', () => {
  const { limiter } = setup({ capacity: 3, refillPerSecond: 1 });
  const bad = [0, -1, 1.5, NaN, Infinity, '1', null];
  for (const n of bad) {
    assert.throws(() => limiter.tryRemove(n), RangeError, `tryRemove(${n})`);
    assert.throws(() => limiter.msUntil(n), RangeError, `msUntil(${n})`);
  }
});

test('tryRemove and msUntil reject n greater than capacity', () => {
  const { limiter } = setup({ capacity: 3, refillPerSecond: 1 });
  assert.throws(() => limiter.tryRemove(4), RangeError);
  assert.throws(() => limiter.msUntil(4), RangeError);
});

test('reset makes the bucket full again', () => {
  const { limiter } = setup({ capacity: 3, refillPerSecond: 1 });
  assert.strictEqual(limiter.tryRemove(3), true);
  limiter.reset();
  assert.strictEqual(limiter.available(), 3);
});

test('clock going backwards adds no tokens and loses none', () => {
  const { clock, limiter } = setup({ capacity: 5, refillPerSecond: 1 });
  clock.t = 1000;
  assert.strictEqual(limiter.tryRemove(3), true);
  assert.strictEqual(limiter.available(), 2);

  clock.t = 0;
  assert.strictEqual(limiter.available(), 2);

  clock.t = 1000;
  assert.strictEqual(limiter.available(), 3);
});
