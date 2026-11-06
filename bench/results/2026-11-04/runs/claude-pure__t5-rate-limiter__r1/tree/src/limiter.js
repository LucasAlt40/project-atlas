'use strict';

function createLimiter(options) {
  const { capacity, refillPerSecond, now = Date.now } = options ?? {};

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
      // Clock went backwards: add nothing, and take the earlier time as the reference.
      last = t;
      return;
    }
    tokens = Math.min(capacity, tokens + ((t - last) * refillPerSecond) / 1000);
    last = t;
  }

  function checkCount(n) {
    if (!Number.isInteger(n) || n < 1) {
      throw new RangeError('n must be a positive integer');
    }
    if (n > capacity) {
      throw new RangeError(`n must not exceed capacity (${capacity})`);
    }
  }

  return {
    tryRemove(n = 1) {
      checkCount(n);
      refill();
      if (tokens < n) {
        return false;
      }
      tokens -= n;
      return true;
    },

    available() {
      refill();
      return Math.floor(tokens);
    },

    msUntil(n = 1) {
      checkCount(n);
      refill();
      if (tokens >= n) {
        return 0;
      }
      return Math.ceil(((n - tokens) * 1000) / refillPerSecond);
    },

    reset() {
      refill();
      tokens = capacity;
    },
  };
}

module.exports = { createLimiter };
