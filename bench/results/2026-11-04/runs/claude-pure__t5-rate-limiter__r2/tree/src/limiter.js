// Tolerance for floating-point drift in token counts, so that a bucket that
// should hold exactly n tokens is not treated as one token short.
const EPSILON = 1e-9;

function checkCount(n, capacity) {
  if (!Number.isInteger(n) || n < 1) {
    throw new RangeError(`n must be a positive integer, got ${n}`);
  }
  if (n > capacity) {
    throw new RangeError(`n (${n}) exceeds capacity (${capacity}) and can never be satisfied`);
  }
}

function createLimiter(options) {
  const { capacity, refillPerSecond, now = Date.now } = options || {};

  if (!Number.isInteger(capacity) || capacity < 1) {
    throw new RangeError(`capacity must be a positive integer, got ${capacity}`);
  }
  if (typeof refillPerSecond !== 'number' || !Number.isFinite(refillPerSecond) || refillPerSecond <= 0) {
    throw new RangeError(`refillPerSecond must be a positive finite number, got ${refillPerSecond}`);
  }

  let tokens = capacity;
  let last = now();

  function refill() {
    const t = now();
    if (t < last) {
      // The clock went backwards: add nothing and take nothing, and treat
      // the earlier time as the new reference point.
      last = t;
      return;
    }
    tokens = Math.min(capacity, tokens + ((t - last) * refillPerSecond) / 1000);
    last = t;
  }

  function tryRemove(n = 1) {
    checkCount(n, capacity);
    refill();
    if (tokens + EPSILON < n) {
      return false;
    }
    tokens -= n;
    return true;
  }

  function available() {
    refill();
    return Math.floor(tokens + EPSILON);
  }

  function msUntil(n = 1) {
    checkCount(n, capacity);
    refill();
    const deficit = n - tokens;
    if (deficit <= EPSILON) {
      return 0;
    }
    return Math.ceil((deficit * 1000) / refillPerSecond - EPSILON);
  }

  function reset() {
    refill();
    tokens = capacity;
  }

  return { tryRemove, available, msUntil, reset };
}

module.exports = { createLimiter };
