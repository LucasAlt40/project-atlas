// Tolerance for floating-point drift when comparing fractional token counts.
const EPS = 1e-9;

function createLimiter(options = {}) {
  const { capacity, refillPerSecond, now = Date.now } = options;

  if (!Number.isInteger(capacity) || capacity <= 0) {
    throw new RangeError('capacity must be a positive integer');
  }
  if (
    typeof refillPerSecond !== 'number' ||
    !Number.isFinite(refillPerSecond) ||
    refillPerSecond <= 0
  ) {
    throw new RangeError('refillPerSecond must be a positive finite number');
  }

  let tokens = capacity;
  let last = now();

  function refill() {
    const t = now();
    if (t < last) {
      // The clock went backwards: move the reference only, so nothing is gained or lost.
      last = t;
      return;
    }
    tokens = Math.min(capacity, tokens + ((t - last) * refillPerSecond) / 1000);
    last = t;
  }

  function assertCount(n) {
    if (!Number.isInteger(n) || n <= 0) {
      throw new RangeError('n must be a positive integer');
    }
    if (n > capacity) {
      throw new RangeError('n must not exceed capacity');
    }
  }

  return {
    tryRemove(n = 1) {
      assertCount(n);
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
      assertCount(n);
      refill();
      const deficit = n - tokens;
      if (deficit <= EPS) {
        return 0;
      }
      return Math.max(1, Math.ceil((deficit * 1000) / refillPerSecond - EPS));
    },

    reset() {
      tokens = capacity;
      last = now();
    },
  };
}

module.exports = { createLimiter };
