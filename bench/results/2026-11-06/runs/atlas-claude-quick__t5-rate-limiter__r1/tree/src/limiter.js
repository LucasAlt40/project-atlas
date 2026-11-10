'use strict';

function isFinitePositive(value) {
  return typeof value === 'number' && Number.isFinite(value) && value > 0;
}

function createLimiter(options) {
  const { capacity, refillPerSecond, now = Date.now } = options ?? {};

  if (!Number.isInteger(capacity) || capacity <= 0) {
    throw new RangeError('capacity must be a positive integer');
  }
  if (!isFinitePositive(refillPerSecond)) {
    throw new RangeError('refillPerSecond must be a finite positive number');
  }

  let tokens = capacity;
  let last = now();

  function refill() {
    const current = now();
    if (current < last) {
      // The clock went backwards: add nothing and take the earlier time as the new reference.
      last = current;
      return;
    }
    tokens = Math.min(capacity, tokens + ((current - last) * refillPerSecond) / 1000);
    last = current;
  }

  function checkCount(n) {
    if (!Number.isInteger(n) || n <= 0) {
      throw new RangeError('n must be a positive integer');
    }
    if (n > capacity) {
      throw new RangeError('n must not exceed capacity');
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
      const deficit = n - tokens;
      if (deficit <= 0) {
        return 0;
      }
      return Math.ceil((deficit * 1000) / refillPerSecond);
    },

    reset() {
      tokens = capacity;
      last = now();
    },
  };
}

module.exports = { createLimiter };
