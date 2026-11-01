'use strict';

function checkCapacity(capacity) {
  if (!Number.isInteger(capacity) || capacity <= 0) {
    throw new RangeError('capacity must be a positive integer');
  }
}

function checkRefillPerSecond(refillPerSecond) {
  if (typeof refillPerSecond !== 'number' || !Number.isFinite(refillPerSecond) || refillPerSecond <= 0) {
    throw new RangeError('refillPerSecond must be a positive finite number');
  }
}

function checkCount(n, capacity) {
  if (!Number.isInteger(n) || n <= 0) {
    throw new RangeError('n must be a positive integer');
  }
  if (n > capacity) {
    throw new RangeError('n must not exceed capacity');
  }
}

function createLimiter(options = {}) {
  const { capacity, refillPerSecond, now = Date.now } = options;

  checkCapacity(capacity);
  checkRefillPerSecond(refillPerSecond);

  let tokens = capacity;
  let last = now();

  function refill() {
    const t = now();
    // A clock that went backwards adds nothing; the earlier time becomes the new reference.
    if (t > last) {
      tokens = Math.min(capacity, tokens + ((t - last) * refillPerSecond) / 1000);
    }
    last = t;
  }

  function tryRemove(n = 1) {
    checkCount(n, capacity);
    refill();
    if (tokens < n) {
      return false;
    }
    tokens -= n;
    return true;
  }

  function available() {
    refill();
    return Math.floor(tokens);
  }

  function msUntil(n = 1) {
    checkCount(n, capacity);
    refill();
    if (tokens >= n) {
      return 0;
    }
    return Math.ceil(((n - tokens) * 1000) / refillPerSecond);
  }

  function reset() {
    tokens = capacity;
    last = now();
  }

  return { tryRemove, available, msUntil, reset };
}

module.exports = { createLimiter };
