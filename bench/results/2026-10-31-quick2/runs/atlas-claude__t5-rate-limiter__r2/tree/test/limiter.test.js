const { test } = require('node:test');
const assert = require('node:assert');
const { createLimiter } = require('../src/limiter');

function makeClock(start = 0) {
  const clock = {
    t: start,
    now: () => clock.t,
  };
  return clock;
}

test('exports only createLimiter', () => {
  assert.deepStrictEqual(Object.keys(require('../src/limiter')), ['createLimiter']);
});

test('starts full', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.available(), 5);
});

test('rejects invalid capacity with RangeError', () => {
  for (const capacity of [0, -1, 1.5, NaN, Infinity, '5', undefined]) {
    assert.throws(() => createLimiter({ capacity, refillPerSecond: 1 }), RangeError, `capacity ${capacity}`);
  }
});

test('rejects invalid refillPerSecond with RangeError', () => {
  for (const refillPerSecond of [0, -1, NaN, Infinity, '1', undefined]) {
    assert.throws(() => createLimiter({ capacity: 5, refillPerSecond }), RangeError, `refill ${refillPerSecond}`);
  }
});

test('rejects missing options with RangeError', () => {
  assert.throws(() => createLimiter(), RangeError);
  assert.throws(() => createLimiter({ capacity: 5 }), RangeError);
});

test('tryRemove removes tokens when available and fails without removing otherwise', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.tryRemove(2), true);
  assert.strictEqual(limiter.tryRemove(2), false);
  assert.strictEqual(limiter.available(), 1);
  assert.strictEqual(limiter.tryRemove(), true);
  assert.strictEqual(limiter.available(), 0);
});

test('tryRemove rejects invalid n with RangeError', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  for (const n of [0, -1, 1.5, NaN, '1']) {
    assert.throws(() => limiter.tryRemove(n), RangeError, `n ${n}`);
  }
});

test('tryRemove throws when n exceeds capacity even if tokens are available', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock.now });
  assert.throws(() => limiter.tryRemove(4), RangeError);
  assert.throws(() => limiter.msUntil(4), RangeError);
  assert.strictEqual(limiter.available(), 3);
});

test('tokens refill continuously and never exceed capacity', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 2, now: clock.now });
  assert.strictEqual(limiter.tryRemove(10), true);
  clock.t = 500;
  assert.strictEqual(limiter.available(), 1);
  clock.t = 1000000;
  assert.strictEqual(limiter.available(), 10);
});

test('available rounds down to an integer', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(5);
  clock.t = 1500;
  assert.strictEqual(limiter.available(), 1);
});

test('fractional refill: 0.5 tokens/s gives exactly one token at 2000 ms', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 0.5, now: clock.now });
  assert.strictEqual(limiter.tryRemove(), true);
  clock.t = 1999;
  assert.strictEqual(limiter.tryRemove(), false);
  assert.strictEqual(limiter.available(), 0);
  clock.t = 2000;
  assert.strictEqual(limiter.tryRemove(), true);
});

test('msUntil returns 0 when tokens are already available', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 4, refillPerSecond: 1, now: clock.now });
  assert.strictEqual(limiter.msUntil(4), 0);
});

test('msUntil counts whole milliseconds until n tokens are available', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 2, now: clock.now });
  limiter.tryRemove(10);
  assert.strictEqual(limiter.msUntil(), 500);
  assert.strictEqual(limiter.msUntil(3), 1500);
  clock.t = 250;
  assert.strictEqual(limiter.msUntil(), 250);
});

test('msUntil rounds up to the next whole millisecond', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 5, refillPerSecond: 3, now: clock.now });
  limiter.tryRemove(5);
  assert.strictEqual(limiter.msUntil(), 334);
});

test('msUntil with fractional rate 0.5 tokens/s gives 1000 ms for one token', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 1, refillPerSecond: 0.5, now: clock.now });
  limiter.tryRemove();
  assert.strictEqual(limiter.msUntil(), 2000);
  clock.t = 1000;
  assert.strictEqual(limiter.msUntil(), 1000);
});

test('reset makes the bucket full again', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(10);
  clock.t = 5000;
  limiter.reset();
  assert.strictEqual(limiter.available(), 10);
  assert.strictEqual(limiter.tryRemove(10), true);
  assert.strictEqual(limiter.available(), 0);
});

test('reset does not carry elapsed time from before the reset', () => {
  const clock = makeClock();
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 1, now: clock.now });
  limiter.tryRemove(10);
  clock.t = 5000;
  limiter.reset();
  clock.t = 5000;
  assert.strictEqual(limiter.tryRemove(10), true);
  clock.t = 5999;
  assert.strictEqual(limiter.available(), 0);
});

test('clock going backwards adds no tokens and loses none', () => {
  const clock = makeClock(1000);
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 2, now: clock.now });
  assert.strictEqual(limiter.tryRemove(10), true);
  clock.t = 2000;
  assert.strictEqual(limiter.available(), 2);
  clock.t = 500;
  assert.strictEqual(limiter.available(), 2);
  assert.strictEqual(limiter.tryRemove(2), true);
  assert.strictEqual(limiter.available(), 0);
});

test('after clock goes backwards, the earlier time becomes the new reference', () => {
  const clock = makeClock(1000);
  const limiter = createLimiter({ capacity: 10, refillPerSecond: 2, now: clock.now });
  limiter.tryRemove(10);
  clock.t = 500;
  assert.strictEqual(limiter.available(), 0);
  clock.t = 1000;
  assert.strictEqual(limiter.available(), 1);
});
