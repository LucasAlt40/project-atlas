'use strict';

const { describe, it } = require('node:test');
const assert = require('node:assert');
const { createLimiter } = require('../src/limiter');

function fakeClock(start = 0) {
  const clock = {
    time: start,
    now: () => clock.time,
    advance: (ms) => {
      clock.time += ms;
    },
  };
  return clock;
}

describe('createLimiter validation', () => {
  it('rejects a missing, non-integer, zero, negative or non-finite capacity', () => {
    for (const capacity of [undefined, null, 0, -1, 1.5, NaN, Infinity, '5']) {
      assert.throws(() => createLimiter({ capacity, refillPerSecond: 1 }), RangeError);
    }
  });

  it('rejects a missing, zero, negative or non-finite refillPerSecond', () => {
    for (const refillPerSecond of [undefined, null, 0, -1, NaN, Infinity, '1']) {
      assert.throws(() => createLimiter({ capacity: 1, refillPerSecond }), RangeError);
    }
  });

  it('accepts a fractional refillPerSecond', () => {
    assert.doesNotThrow(() => createLimiter({ capacity: 1, refillPerSecond: 0.5 }));
  });

  it('rejects an invalid n in tryRemove and msUntil', () => {
    const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
    for (const n of [0, -1, 1.5, NaN, Infinity, '1']) {
      assert.throws(() => limiter.tryRemove(n), RangeError);
      assert.throws(() => limiter.msUntil(n), RangeError);
    }
  });

  it('rejects an n larger than capacity in tryRemove and msUntil', () => {
    const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
    assert.throws(() => limiter.tryRemove(4), RangeError);
    assert.throws(() => limiter.msUntil(4), RangeError);
  });
});

describe('createLimiter token bucket', () => {
  it('starts full', () => {
    const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: fakeClock().now });
    assert.strictEqual(limiter.available(), 5);
  });

  it('removes one token by default', () => {
    const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: fakeClock().now });
    assert.strictEqual(limiter.tryRemove(), true);
    assert.strictEqual(limiter.available(), 1);
  });

  it('removes n tokens when enough are available', () => {
    const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: fakeClock().now });
    assert.strictEqual(limiter.tryRemove(3), true);
    assert.strictEqual(limiter.available(), 2);
  });

  it('returns false and removes nothing when not enough tokens are available', () => {
    const limiter = createLimiter({ capacity: 3, refillPerSecond: 1, now: fakeClock().now });
    assert.strictEqual(limiter.tryRemove(2), true);
    assert.strictEqual(limiter.tryRemove(2), false);
    assert.strictEqual(limiter.available(), 1);
  });

  it('refills continuously and reports whole tokens rounded down', () => {
    const clock = fakeClock();
    const limiter = createLimiter({ capacity: 10, refillPerSecond: 2, now: clock.now });
    assert.strictEqual(limiter.tryRemove(10), true);
    clock.advance(250);
    assert.strictEqual(limiter.available(), 0);
    clock.advance(250);
    assert.strictEqual(limiter.available(), 1);
  });

  it('never refills above capacity', () => {
    const clock = fakeClock();
    const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
    limiter.tryRemove(1);
    clock.advance(100000);
    assert.strictEqual(limiter.available(), 5);
  });

  it('supports a fractional refill rate', () => {
    const clock = fakeClock();
    const limiter = createLimiter({ capacity: 1, refillPerSecond: 0.5, now: clock.now });
    assert.strictEqual(limiter.tryRemove(), true);
    clock.advance(1000);
    assert.strictEqual(limiter.tryRemove(), false);
    clock.advance(1000);
    assert.strictEqual(limiter.tryRemove(), true);
  });

  it('uses Date.now when no now function is given', () => {
    const limiter = createLimiter({ capacity: 2, refillPerSecond: 1 });
    assert.strictEqual(limiter.available(), 2);
    assert.strictEqual(limiter.tryRemove(2), true);
    assert.strictEqual(limiter.tryRemove(), false);
  });
});

describe('createLimiter msUntil', () => {
  it('returns 0 when the tokens are already available', () => {
    const limiter = createLimiter({ capacity: 2, refillPerSecond: 1, now: fakeClock().now });
    assert.strictEqual(limiter.msUntil(), 0);
    assert.strictEqual(limiter.msUntil(2), 0);
  });

  it('returns the milliseconds until n tokens are available', () => {
    const clock = fakeClock();
    const limiter = createLimiter({ capacity: 5, refillPerSecond: 2, now: clock.now });
    limiter.tryRemove(5);
    assert.strictEqual(limiter.msUntil(1), 500);
    assert.strictEqual(limiter.msUntil(3), 1500);
  });

  it('accounts for tokens that have already partly refilled', () => {
    const clock = fakeClock();
    const limiter = createLimiter({ capacity: 5, refillPerSecond: 2, now: clock.now });
    limiter.tryRemove(5);
    clock.advance(100);
    assert.strictEqual(limiter.msUntil(1), 400);
  });

  it('rounds the wait up to a whole millisecond', () => {
    const clock = fakeClock();
    const limiter = createLimiter({ capacity: 1, refillPerSecond: 3, now: clock.now });
    limiter.tryRemove();
    assert.strictEqual(limiter.msUntil(), 334);
  });

  it('agrees with tryRemove at the moment the wait ends', () => {
    const clock = fakeClock();
    const limiter = createLimiter({ capacity: 1, refillPerSecond: 3, now: clock.now });
    limiter.tryRemove();
    assert.strictEqual(limiter.tryRemove(), false);
    clock.advance(limiter.msUntil());
    assert.strictEqual(limiter.tryRemove(), true);
  });
});

describe('createLimiter reset', () => {
  it('makes the bucket full again', () => {
    const limiter = createLimiter({ capacity: 4, refillPerSecond: 1, now: fakeClock().now });
    limiter.tryRemove(4);
    limiter.reset();
    assert.strictEqual(limiter.available(), 4);
  });

  it('starts a fresh refill reference at the time of reset', () => {
    const clock = fakeClock();
    const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
    limiter.tryRemove(5);
    clock.advance(3000);
    limiter.reset();
    assert.strictEqual(limiter.tryRemove(5), true);
    clock.advance(500);
    assert.strictEqual(limiter.available(), 0);
    assert.strictEqual(limiter.msUntil(1), 500);
  });
});

describe('createLimiter clock going backwards', () => {
  it('adds and loses no tokens when the clock moves earlier', () => {
    const clock = fakeClock(10000);
    const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
    limiter.tryRemove(2);
    clock.time = 9000;
    assert.strictEqual(limiter.available(), 3);
  });

  it('treats the earlier time as the new reference', () => {
    const clock = fakeClock(10000);
    const limiter = createLimiter({ capacity: 5, refillPerSecond: 1, now: clock.now });
    limiter.tryRemove(2);
    clock.time = 9000;
    assert.strictEqual(limiter.available(), 3);
    clock.time = 10000;
    assert.strictEqual(limiter.available(), 4);
  });
});
