// Tolerance for floating point error when flooring or ceiling token counts.
const EPSILON = 1e-9;

function assertCount(n, capacity) {
  if (!Number.isInteger(n) || n <= 0) {
    throw new RangeError(`n must be a positive integer, got ${n}`);
  }
  if (n > capacity) {
    throw new RangeError(`n (${n}) exceeds capacity (${capacity})`);
  }
}

function createLimiter(options = {}) {
  const { capacity, refillPerSecond, now = Date.now } = options;

  if (!Number.isInteger(capacity) || capacity <= 0) {
    throw new RangeError(`capacity must be a positive integer, got ${capacity}`);
  }
  if (!Number.isFinite(refillPerSecond) || refillPerSecond <= 0) {
    throw new RangeError(`refillPerSecond must be a positive finite number, got ${refillPerSecond}`);
  }

  let tokens = capacity;
  let last = now();

  function refill() {
    const current = now();
    if (current < last) {
      // Clock went backwards: add nothing, and use the earlier time as the new reference.
      last = current;
      return;
    }
    tokens = Math.min(capacity, tokens + ((current - last) * refillPerSecond) / 1000);
    last = current;
  }

  return {
    tryRemove(n = 1) {
      assertCount(n, capacity);
      refill();
      if (tokens + EPSILON < n) {
        return false;
      }
      tokens = Math.max(0, tokens - n);
      return true;
    },

    available() {
      refill();
      return Math.floor(tokens + EPSILON);
    },

    msUntil(n = 1) {
      assertCount(n, capacity);
      refill();
      const deficit = n - tokens;
      if (deficit <= EPSILON) {
        return 0;
      }
      return Math.ceil((deficit * 1000) / refillPerSecond - EPSILON);
    },

    reset() {
      tokens = capacity;
    },
  };
}

module.exports = { createLimiter };
