'use strict';

function isPositiveNumber(value) {
  return typeof value === 'number' && Number.isFinite(value) && value > 0;
}

function checkCount(n) {
  if (!Number.isInteger(n) || n < 1) {
    throw new RangeError('n must be a positive integer');
  }
}

function createLimiter(options) {
  const { capacity, refillPerSecond, now = Date.now } = options || {};

  if (!Number.isInteger(capacity) || capacity < 1) {
    throw new RangeError('capacity must be a positive integer');
  }
  if (!isPositiveNumber(refillPerSecond)) {
    throw new RangeError('refillPerSecond must be a positive number');
  }

  let tokens = capacity;
  let last = now();

  // The only place the clock is read. A backwards clock adds nothing and
  // becomes the new reference time, so no tokens are lost either.
  function refill() {
    const t = now();
    if (t > last) {
      tokens = Math.min(capacity, tokens + ((t - last) * refillPerSecond) / 1000);
    }
    last = t;
  }

  function checkRequest(n) {
    checkCount(n);
    if (n > capacity) {
      throw new RangeError('n must not exceed capacity');
    }
  }

  return {
    tryRemove(n = 1) {
      checkRequest(n);
      refill();
      if (tokens >= n) {
        tokens -= n;
        return true;
      }
      return false;
    },

    available() {
      refill();
      return Math.floor(tokens);
    },

    msUntil(n = 1) {
      checkRequest(n);
      refill();
      if (tokens >= n) {
        return 0;
      }
      return Math.ceil(((n - tokens) * 1000) / refillPerSecond);
    },

    reset() {
      tokens = capacity;
      last = now();
    },
  };
}

module.exports = { createLimiter };
