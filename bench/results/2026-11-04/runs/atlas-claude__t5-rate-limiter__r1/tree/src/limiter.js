'use strict';

function assertCount(n) {
  if (!Number.isInteger(n) || n <= 0) {
    throw new RangeError('n must be a positive integer');
  }
}

function createLimiter(options) {
  const { capacity, refillPerSecond, now = Date.now } = options || {};

  if (!Number.isInteger(capacity) || capacity <= 0) {
    throw new RangeError('capacity must be a positive integer');
  }
  if (typeof refillPerSecond !== 'number' || !Number.isFinite(refillPerSecond) || refillPerSecond <= 0) {
    throw new RangeError('refillPerSecond must be a positive finite number');
  }

  let tokens = capacity;
  let last = now();

  // Adds tokens for the time elapsed since the last reference point. A clock
  // that went backwards adds nothing, and the earlier time becomes the new reference.
  function refill() {
    const t = now();
    if (t > last) {
      tokens = Math.min(capacity, tokens + ((t - last) * refillPerSecond) / 1000);
    }
    last = t;
  }

  function assertFits(n) {
    assertCount(n);
    if (n > capacity) {
      throw new RangeError('n must not exceed capacity');
    }
  }

  return {
    tryRemove(n = 1) {
      assertFits(n);
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
      assertFits(n);
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
