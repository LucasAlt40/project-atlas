// Tolerance for floating point drift when refilling fractional tokens.
const EPS = 1e-9;

function createLimiter(options = {}) {
  const { capacity, refillPerSecond, now = Date.now } = options ?? {};

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
    // Clock went backwards: keep the tokens and move the reference time.
    if (t < last) {
      last = t;
      return;
    }
    tokens = Math.min(capacity, tokens + ((t - last) / 1000) * refillPerSecond);
    last = t;
  }

  function assertCount(n) {
    if (!Number.isInteger(n) || n <= 0) {
      throw new RangeError('n must be a positive integer');
    }
    if (n > capacity) {
      throw new RangeError(`n must not exceed capacity (${capacity})`);
    }
  }

  return {
    tryRemove(n = 1) {
      assertCount(n);
      refill();
      if (tokens + EPS >= n) {
        tokens = Math.max(0, tokens - n);
        return true;
      }
      return false;
    },

    available() {
      refill();
      return Math.floor(tokens + EPS);
    },

    msUntil(n = 1) {
      assertCount(n);
      refill();
      const deficit = n - tokens;
      if (deficit <= EPS) {
        return 0;
      }
      return Math.ceil((deficit / refillPerSecond) * 1000 - EPS);
    },

    reset() {
      refill();
      tokens = capacity;
    },
  };
}

module.exports = { createLimiter };
