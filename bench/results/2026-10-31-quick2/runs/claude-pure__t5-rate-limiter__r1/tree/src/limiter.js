'use strict';

function isPositiveFiniteNumber(value) {
  return typeof value === 'number' && Number.isFinite(value) && value > 0;
}

function checkTokenCount(n) {
  if (!Number.isInteger(n) || n <= 0) {
    throw new RangeError(`token count must be a positive integer, got ${n}`);
  }
}

function createLimiter(options) {
  const { capacity, refillPerSecond, now = Date.now } = options;

  if (!Number.isInteger(capacity) || capacity <= 0) {
    throw new RangeError(`capacity must be a positive integer, got ${capacity}`);
  }
  if (!isPositiveFiniteNumber(refillPerSecond)) {
    throw new RangeError(`refillPerSecond must be a positive finite number, got ${refillPerSecond}`);
  }

  let tokens = capacity;
  let last = now();

  function refill() {
    const t = now();
    // A clock that went backwards adds nothing and loses nothing; the earlier time becomes the reference.
    if (t > last) {
      tokens = Math.min(capacity, tokens + ((t - last) * refillPerSecond) / 1000);
    }
    last = t;
  }

  function checkRequest(n) {
    checkTokenCount(n);
    if (n > capacity) {
      throw new RangeError(`requested ${n} tokens but capacity is ${capacity}`);
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
