// Tolerance for floating-point drift in fractional refills.
const EPS = 1e-9;

function createLimiter(options = {}) {
  const { capacity, refillPerSecond, now = Date.now } = options;

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
      // The clock went backwards: add nothing and make the earlier time the new reference.
      // Known trade-off: the span between t and the old reference can be credited again later.
      last = t;
      return;
    }
    tokens = Math.min(capacity, tokens + ((t - last) / 1000) * refillPerSecond);
    last = t;
  }

  function checkCount(n) {
    if (!Number.isInteger(n) || n <= 0 || n > capacity) {
      throw new RangeError(`n must be a positive integer no greater than ${capacity}`);
    }
  }

  function tryRemove(n = 1) {
    checkCount(n);
    refill();
    if (tokens + EPS < n) {
      return false;
    }
    tokens = Math.max(0, tokens - n);
    return true;
  }

  function available() {
    refill();
    return Math.floor(tokens + EPS);
  }

  function msUntil(n = 1) {
    checkCount(n);
    refill();
    if (tokens + EPS >= n) {
      return 0;
    }
    return Math.max(1, Math.ceil(((n - tokens) / refillPerSecond) * 1000 - EPS));
  }

  function reset() {
    tokens = capacity;
    last = now();
  }

  return { tryRemove, available, msUntil, reset };
}

module.exports = { createLimiter };
