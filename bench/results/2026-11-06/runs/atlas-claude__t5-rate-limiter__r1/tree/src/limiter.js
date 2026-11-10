// Token bucket. Tokens refill continuously at refillPerSecond, up to capacity.
// EPS absorbs floating point error so that msUntil(n) followed by tryRemove(n)
// succeeds exactly at the returned time.
const EPS = 1e-9;

function createLimiter(options = {}) {
  const { capacity, refillPerSecond, now = Date.now } = options;

  if (!Number.isInteger(capacity) || capacity <= 0) {
    throw new RangeError('capacity must be a positive integer');
  }
  if (typeof refillPerSecond !== 'number' || !Number.isFinite(refillPerSecond) || refillPerSecond <= 0) {
    throw new RangeError('refillPerSecond must be a positive finite number');
  }
  if (typeof now !== 'function') {
    throw new TypeError('now must be a function');
  }

  let tokens = capacity;
  let last = now();

  function refill() {
    const t = now();
    // If the clock went backwards, add nothing and make the earlier time the reference.
    // Note: time between the regressed clock and the previous maximum is credited again
    // once the clock moves forward past it.
    if (t > last) {
      tokens = Math.min(capacity, tokens + ((t - last) / 1000) * refillPerSecond);
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
    // Tokens are short, so the wait is at least 1 ms even when the shortfall is within EPS.
    return Math.max(1, Math.ceil(((n - tokens) / refillPerSecond) * 1000 - EPS));
  }

  function reset() {
    refill();
    tokens = capacity;
  }

  return { tryRemove, available, msUntil, reset };
}

module.exports = { createLimiter };
