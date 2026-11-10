'use strict';

// Absorbs floating-point drift from the continuous refill so that a bucket
// that should hold exactly n tokens does not report n - 1e-12.
const EPSILON = 1e-9;

function createLimiter({ capacity, refillPerSecond, now = Date.now } = {}) {
  if (!Number.isInteger(capacity) || capacity < 1) {
    throw new RangeError(`capacity must be a positive integer, got ${capacity}`);
  }
  if (!Number.isFinite(refillPerSecond) || refillPerSecond <= 0) {
    throw new RangeError(`refillPerSecond must be a positive finite number, got ${refillPerSecond}`);
  }

  let tokens = capacity;
  let last = now();

  function refill() {
    const t = now();
    // If the clock went backwards, add nothing and treat the earlier time as
    // the new reference, so no tokens are added or lost.
    if (t > last) {
      tokens = Math.min(capacity, tokens + ((t - last) * refillPerSecond) / 1000);
    }
    last = t;
  }

  function checkCount(n) {
    if (!Number.isInteger(n) || n < 1) {
      throw new RangeError(`n must be a positive integer, got ${n}`);
    }
    if (n > capacity) {
      throw new RangeError(`n (${n}) exceeds capacity (${capacity}) and can never be satisfied`);
    }
  }

  return {
    tryRemove(n = 1) {
      checkCount(n);
      refill();
      if (tokens + EPSILON < n) {
        return false;
      }
      tokens = Math.max(0, tokens - n);
      return true;
    },

    available() {
      refill();
      return Math.floor(tokens + EPSILON);
    },

    msUntil(n = 1) {
      checkCount(n);
      refill();
      const deficit = n - tokens;
      if (deficit <= EPSILON) {
        return 0;
      }
      return Math.ceil((deficit * 1000) / refillPerSecond);
    },

    reset() {
      tokens = capacity;
    },
  };
}

module.exports = { createLimiter };
