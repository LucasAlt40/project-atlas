'use strict';

// Tolerance for floating point drift when comparing token counts.
// available(), tryRemove() and msUntil() all use it so they agree.
const EPSILON = 1e-9;

function isPositiveInteger(value) {
  return Number.isInteger(value) && value > 0;
}

function isPositiveFiniteNumber(value) {
  return typeof value === 'number' && Number.isFinite(value) && value > 0;
}

function checkCount(n, capacity) {
  if (!isPositiveInteger(n)) {
    throw new RangeError('n must be a positive integer');
  }
  if (n > capacity) {
    throw new RangeError(`n (${n}) exceeds capacity (${capacity})`);
  }
}

function createLimiter(options) {
  const { capacity, refillPerSecond, now = Date.now } = options || {};

  if (!isPositiveInteger(capacity)) {
    throw new RangeError('capacity must be a positive integer');
  }
  if (!isPositiveFiniteNumber(refillPerSecond)) {
    throw new RangeError('refillPerSecond must be a positive finite number');
  }
  if (typeof now !== 'function') {
    throw new TypeError('now must be a function');
  }

  let tokens = capacity;
  let last = now();

  function refill() {
    const current = now();
    if (current < last) {
      // Clock went backwards: add nothing, and use the earlier time as the reference.
      last = current;
      return;
    }
    tokens = Math.min(capacity, tokens + ((current - last) / 1000) * refillPerSecond);
    last = current;
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
    const deficit = n - tokens;
    if (deficit <= EPSILON) {
      return 0;
    }
    return Math.ceil((deficit / refillPerSecond) * 1000);
  }

  function reset() {
    tokens = capacity;
    last = now();
  }

  return { tryRemove, available, msUntil, reset };
}

module.exports = { createLimiter };
