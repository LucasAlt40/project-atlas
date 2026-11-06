// Tolerance for float error from fractional refill rates (e.g. 0.3 tokens/s).
const EPS = 1e-9;

function assertCount(n, capacity) {
  if (!Number.isInteger(n) || n <= 0) {
    throw new RangeError('n must be a positive integer');
  }
  if (n > capacity) {
    throw new RangeError('n must not exceed capacity');
  }
}

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

  // Adds tokens for the time elapsed since the last call. If the clock went
  // backwards, nothing is added and the earlier time becomes the reference.
  function refill() {
    const t = now();
    if (t > last) {
      tokens = Math.min(capacity, tokens + ((t - last) * refillPerSecond) / 1000);
    }
    last = t;
  }

  return {
    tryRemove(n = 1) {
      assertCount(n, capacity);
      refill();
      if (tokens + EPS < n) {
        return false;
      }
      tokens = Math.max(0, tokens - n);
      return true;
    },

    available() {
      refill();
      return Math.floor(tokens + EPS);
    },

    msUntil(n = 1) {
      assertCount(n, capacity);
      refill();
      const deficit = n - tokens;
      if (deficit <= EPS) {
        return 0;
      }
      return Math.ceil((deficit * 1000) / refillPerSecond - EPS);
    },

    reset() {
      refill();
      tokens = capacity;
    },
  };
}

module.exports = { createLimiter };
