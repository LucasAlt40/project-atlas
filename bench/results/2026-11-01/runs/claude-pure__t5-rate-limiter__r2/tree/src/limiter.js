'use strict';

// Float tolerance: refill arithmetic is not exact, so a bucket that is
// mathematically at 3 tokens may hold 2.9999999999999996.
const EPSILON = 1e-9;

function checkCount(n, capacity) {
  if (!Number.isInteger(n) || n <= 0) {
    throw new RangeError(`n must be a positive integer, got ${n}`);
  }
  if (n > capacity) {
    throw new RangeError(`n (${n}) exceeds capacity (${capacity}) and can never succeed`);
  }
}

function createLimiter(options) {
  const { capacity, refillPerSecond, now = Date.now } = options ?? {};

  if (!Number.isInteger(capacity) || capacity <= 0) {
    throw new RangeError(`capacity must be a positive integer, got ${capacity}`);
  }
  if (typeof refillPerSecond !== 'number' || !Number.isFinite(refillPerSecond) || refillPerSecond <= 0) {
    throw new RangeError(`refillPerSecond must be a positive finite number, got ${refillPerSecond}`);
  }

  let tokens = capacity;
  let last = null;

  function refill() {
    const t = now();
    if (last === null || t < last) {
      // First observation, or the clock went backwards: no tokens are added
      // and the current time becomes the reference point.
      last = t;
      return;
    }
    tokens = Math.min(capacity, tokens + ((t - last) * refillPerSecond) / 1000);
    last = t;
  }

  function tryRemove(n = 1) {
    checkCount(n, capacity);
    refill();
    if (tokens + EPSILON < n) {
      return false;
    }
    tokens = Math.max(0, tokens - n);
    return true;
  }

  function available() {
    refill();
    return Math.floor(tokens + EPSILON);
  }

  function msUntil(n = 1) {
    checkCount(n, capacity);
    refill();
    if (tokens + EPSILON >= n) {
      return 0;
    }
    return Math.max(1, Math.ceil(((n - tokens) * 1000) / refillPerSecond - EPSILON));
  }

  function reset() {
    tokens = capacity;
  }

  return { tryRemove, available, msUntil, reset };
}

module.exports = { createLimiter };
