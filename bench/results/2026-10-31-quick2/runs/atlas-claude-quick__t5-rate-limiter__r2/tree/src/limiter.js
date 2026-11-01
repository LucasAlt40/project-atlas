'use strict';

// Tolerance for float drift when comparing token counts.
const EPS = 1e-9;

function assertPositiveNumber(value, name) {
  if (typeof value !== 'number' || !Number.isFinite(value) || value <= 0) {
    throw new RangeError(`${name} must be a positive number`);
  }
}

function assertCount(n, capacity) {
  if (!Number.isInteger(n) || n <= 0) {
    throw new RangeError('n must be a positive integer');
  }
  if (n > capacity) {
    throw new RangeError(`n must not exceed capacity (${capacity})`);
  }
}

function createLimiter(options) {
  const { capacity, refillPerSecond, now = Date.now } = options ?? {};

  if (!Number.isInteger(capacity) || capacity <= 0) {
    throw new RangeError('capacity must be a positive integer');
  }
  assertPositiveNumber(refillPerSecond, 'refillPerSecond');

  let tokens = capacity;
  let last = now();

  function refill() {
    const current = now();
    // A clock that went backwards adds nothing; the earlier time becomes the reference.
    if (current > last) {
      tokens = Math.min(capacity, tokens + ((current - last) / 1000) * refillPerSecond);
    }
    last = current;
  }

  function tryRemove(n = 1) {
    assertCount(n, capacity);
    refill();
    if (tokens + EPS >= n) {
      tokens = Math.max(0, tokens - n);
      return true;
    }
    return false;
  }

  function available() {
    refill();
    return Math.floor(tokens + EPS);
  }

  function msUntil(n = 1) {
    assertCount(n, capacity);
    refill();
    if (tokens + EPS >= n) {
      return 0;
    }
    return Math.ceil(((n - tokens) / refillPerSecond) * 1000);
  }

  function reset() {
    tokens = capacity;
    last = now();
  }

  return { tryRemove, available, msUntil, reset };
}

module.exports = { createLimiter };
