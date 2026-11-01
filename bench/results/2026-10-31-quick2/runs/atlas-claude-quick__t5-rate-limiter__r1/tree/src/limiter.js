'use strict';

// Tolerance for float rounding when comparing token counts.
const EPS = 1e-9;

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
    const t = now();
    if (t < last) {
      // Clock went backwards: add nothing, and use the earlier time as the new reference.
      last = t;
      return;
    }
    // Multiply before dividing so exact boundaries are not lost to float error.
    tokens = Math.min(capacity, tokens + ((t - last) * refillPerSecond) / 1000);
    last = t;
  }

  function checkCount(n) {
    if (!Number.isInteger(n) || n <= 0) {
      throw new RangeError('n must be a positive integer');
    }
    if (n > capacity) {
      throw new RangeError('n must not exceed capacity');
    }
  }

  function tryRemove(n = 1) {
    checkCount(n);
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
    checkCount(n);
    refill();
    const deficit = n - tokens;
    if (deficit <= EPS) {
      return 0;
    }
    return Math.ceil((deficit * 1000) / refillPerSecond - EPS);
  }

  function reset() {
    tokens = capacity;
    last = now();
  }

  return { tryRemove, available, msUntil, reset };
}

module.exports = { createLimiter };
