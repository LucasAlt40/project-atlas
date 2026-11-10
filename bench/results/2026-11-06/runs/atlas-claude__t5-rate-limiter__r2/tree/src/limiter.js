'use strict';

// Tolerance for floating-point drift when comparing token counts and times.
const EPSILON = 1e-9;

function checkCount(n, capacity) {
  if (!Number.isInteger(n) || n <= 0) {
    throw new RangeError('n must be a positive integer');
  }
  if (n > capacity) {
    throw new RangeError(`n (${n}) exceeds capacity (${capacity})`);
  }
}

function createLimiter(options = {}) {
  const { capacity, refillPerSecond, now = Date.now } = options;

  if (!Number.isInteger(capacity) || capacity <= 0) {
    throw new RangeError('capacity must be a positive integer');
  }
  if (typeof refillPerSecond !== 'number' || !Number.isFinite(refillPerSecond) || refillPerSecond <= 0) {
    throw new RangeError('refillPerSecond must be a positive finite number');
  }

  let tokens = capacity;
  let last = now();

  function refill() {
    const time = now();
    // Clock went backwards: keep the tokens and make the earlier time the reference.
    if (time < last) {
      last = time;
      return;
    }
    tokens = Math.min(capacity, tokens + ((time - last) / 1000) * refillPerSecond);
    last = time;
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
    // At least 1 ms is needed while any deficit remains.
    return Math.max(1, Math.ceil((deficit * 1000) / refillPerSecond - EPSILON));
  }

  function reset() {
    tokens = capacity;
    last = now();
  }

  return { tryRemove, available, msUntil, reset };
}

module.exports = { createLimiter };
