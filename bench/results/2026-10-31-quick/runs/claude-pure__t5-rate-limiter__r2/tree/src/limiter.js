function checkCount(n, capacity) {
  if (!Number.isInteger(n) || n <= 0) {
    throw new RangeError(`n must be a positive integer, got ${n}`);
  }
  if (n > capacity) {
    throw new RangeError(`n (${n}) exceeds capacity (${capacity}) and can never be satisfied`);
  }
}

function createLimiter(options) {
  const { capacity, refillPerSecond, now = Date.now } = options ?? {};

  if (!Number.isInteger(capacity) || capacity <= 0) {
    throw new RangeError(`capacity must be a positive integer, got ${capacity}`);
  }
  if (!Number.isFinite(refillPerSecond) || refillPerSecond <= 0) {
    throw new RangeError(`refillPerSecond must be a positive finite number, got ${refillPerSecond}`);
  }

  let tokens = capacity;
  let last = now();

  function refill() {
    const t = now();
    // A clock that moved backwards adds nothing; its time becomes the new reference.
    if (t > last) {
      tokens = Math.min(capacity, tokens + ((t - last) * refillPerSecond) / 1000);
    }
    last = t;
  }

  return {
    tryRemove(n = 1) {
      checkCount(n, capacity);
      refill();
      if (tokens < n) {
        return false;
      }
      tokens -= n;
      return true;
    },

    available() {
      refill();
      return Math.floor(tokens);
    },

    msUntil(n = 1) {
      checkCount(n, capacity);
      refill();
      if (tokens >= n) {
        return 0;
      }
      return Math.ceil(((n - tokens) * 1000) / refillPerSecond);
    },

    reset() {
      tokens = capacity;
    },
  };
}

module.exports = { createLimiter };
