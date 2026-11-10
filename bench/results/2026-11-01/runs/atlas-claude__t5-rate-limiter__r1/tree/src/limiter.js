'use strict';

// Tolerance in tokens (and ms for msUntil) that absorbs floating-point error
// at exact refill boundaries, so a token due at time T is not missed by 1e-15.
const EPS = 1e-9;

function isPositiveNumber(value) {
  return typeof value === 'number' && Number.isFinite(value) && value > 0;
}

function checkCount(n, capacity) {
  if (!Number.isInteger(n) || n <= 0) {
    throw new RangeError('n must be a positive integer');
  }
  if (n > capacity) {
    throw new RangeError(`n (${n}) exceeds capacity (${capacity})`);
  }
}

function createLimiter(options = {}) {
  const { capacity, refillPerSecond } = options;

  if (!Number.isInteger(capacity) || capacity <= 0) {
    throw new RangeError('capacity must be a positive integer');
  }
  if (!isPositiveNumber(refillPerSecond)) {
    throw new RangeError('refillPerSecond must be a positive finite number');
  }

  const now = options.now === undefined ? Date.now : options.now;
  if (typeof now !== 'function') {
    throw new TypeError('now must be a function');
  }

  function readClock() {
    const time = now();
    if (!Number.isFinite(time)) {
      throw new TypeError('now() must return a finite number');
    }
    return time;
  }

  let tokens = capacity;
  let last = readClock();

  function refill() {
    const time = readClock();
    // A clock that went backwards adds nothing; the earlier time becomes the reference.
    if (time > last) {
      tokens = Math.min(capacity, tokens + ((time - last) / 1000) * refillPerSecond);
    }
    last = time;
  }

  function tryRemove(n = 1) {
    checkCount(n, capacity);
    refill();
    if (tokens + EPS >= n) {
      tokens = Math.max(0, tokens - n);
      return true;
    }
    return false;
  }

  function available() {
    refill();
    return Math.floor(tokens + EPS);
  }

  function msUntil(n = 1) {
    checkCount(n, capacity);
    refill();
    const deficit = n - tokens;
    if (deficit <= EPS) {
      return 0;
    }
    return Math.max(1, Math.ceil((deficit * 1000) / refillPerSecond - EPS));
  }

  function reset() {
    tokens = capacity;
  }

  return { tryRemove, available, msUntil, reset };
}

module.exports = { createLimiter };
