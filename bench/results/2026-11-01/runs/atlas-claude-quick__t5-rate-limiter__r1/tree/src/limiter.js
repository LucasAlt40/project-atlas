'use strict';

// Tolerance for floating-point drift from fractional refills.
const EPS = 1e-9;

function checkCount(n, capacity) {
  if (!Number.isInteger(n) || n < 1) {
    throw new RangeError(`n must be a positive integer, got ${n}`);
  }
  if (n > capacity) {
    throw new RangeError(`n (${n}) can never exceed capacity (${capacity})`);
  }
}

function createLimiter(options = {}) {
  const { capacity, refillPerSecond, now = Date.now } = options;

  if (!Number.isInteger(capacity) || capacity <= 0) {
    throw new RangeError(`capacity must be a positive integer, got ${capacity}`);
  }
  if (typeof refillPerSecond !== 'number' || !Number.isFinite(refillPerSecond) || refillPerSecond <= 0) {
    throw new RangeError(`refillPerSecond must be a positive finite number, got ${refillPerSecond}`);
  }

  let tokens = capacity;
  let last = now();

  function refill() {
    const t = now();
    // Clock went backwards: add nothing, and treat the earlier time as the new reference.
    if (t < last) {
      last = t;
      return;
    }
    tokens = Math.min(capacity, tokens + ((t - last) / 1000) * refillPerSecond);
    last = t;
  }

  function tryRemove(n = 1) {
    checkCount(n, capacity);
    refill();
    if (tokens + EPS < n) {
      return false;
    }
    tokens = Math.max(0, tokens - n);
    return true;
  }

  function available() {
    refill();
    return Math.floor(tokens + EPS);
  }

  function msUntil(n = 1) {
    checkCount(n, capacity);
    refill();
    const deficit = n - tokens;
    if (deficit <= EPS) {
      return 0;
    }
    return Math.max(1, Math.ceil((deficit / refillPerSecond) * 1000 - EPS));
  }

  function reset() {
    tokens = capacity;
    last = now();
  }

  return { tryRemove, available, msUntil, reset };
}

module.exports = { createLimiter };
