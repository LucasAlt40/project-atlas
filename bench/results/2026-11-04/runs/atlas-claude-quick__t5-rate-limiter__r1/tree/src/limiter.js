'use strict';

function createLimiter(options) {
  const opts = options || {};
  const capacity = opts.capacity;
  const refillPerSecond = opts.refillPerSecond;
  const now = opts.now === undefined ? Date.now : opts.now;

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
    // A clock that goes backwards adds nothing; the earlier time becomes the reference.
    if (t > last) {
      tokens = Math.min(capacity, tokens + ((t - last) * refillPerSecond) / 1000);
    }
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
    if (tokens >= n) {
      tokens -= n;
      return true;
    }
    return false;
  }

  function available() {
    refill();
    return Math.floor(tokens);
  }

  function msUntil(n = 1) {
    checkCount(n);
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
