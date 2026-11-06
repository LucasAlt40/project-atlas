// Token bucket rate limiter. Tokens refill continuously; the bucket never holds more than `capacity`.

// Tolerates floating-point drift from fractional refill (e.g. 0.1 + 0.1 + ... = 0.9999999999999999).
const EPSILON = 1e-9;

function isPositiveNumber(value) {
  return typeof value === 'number' && Number.isFinite(value) && value > 0;
}

function checkCount(n, capacity) {
  if (!Number.isInteger(n) || n <= 0) {
    throw new RangeError(`n must be a positive integer, got ${n}`);
  }
  if (n > capacity) {
    throw new RangeError(`n (${n}) exceeds capacity (${capacity}) and can never be satisfied`);
  }
}

function createLimiter(options = {}) {
  const { capacity, refillPerSecond, now = Date.now } = options;

  if (!Number.isInteger(capacity) || capacity <= 0) {
    throw new RangeError(`capacity must be a positive integer, got ${capacity}`);
  }
  if (!isPositiveNumber(refillPerSecond)) {
    throw new RangeError(`refillPerSecond must be a positive finite number, got ${refillPerSecond}`);
  }

  let tokens = capacity;
  let last = now();

  function refill() {
    const time = now();
    const elapsed = time - last;
    // A backwards clock adds nothing; the earlier time just becomes the new reference.
    if (elapsed > 0) {
      tokens = Math.min(capacity, tokens + (elapsed / 1000) * refillPerSecond);
    }
    last = time;
  }

  function tryRemove(n = 1) {
    checkCount(n, capacity);
    refill();
    if (tokens + EPSILON >= n) {
      tokens = Math.max(0, tokens - n);
      return true;
    }
    return false;
  }

  function available() {
    refill();
    return Math.floor(tokens + EPSILON);
  }

  function msUntil(n = 1) {
    checkCount(n, capacity);
    refill();
    if (tokens + EPSILON >= n) {
      return 0;
    }
    const deficit = n - tokens;
    return Math.ceil((deficit * 1000) / refillPerSecond - EPSILON);
  }

  function reset() {
    tokens = capacity;
    last = now();
  }

  return { tryRemove, available, msUntil, reset };
}

module.exports = { createLimiter };
