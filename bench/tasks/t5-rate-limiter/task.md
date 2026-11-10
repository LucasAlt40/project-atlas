Create src/limiter.js, a CommonJS module that exports createLimiter(options), exactly as: module.exports = { createLimiter }. It is a token bucket.

options:
- capacity: a positive integer, the most tokens the bucket holds. It starts full.
- refillPerSecond: a positive number (it may be fractional, e.g. 0.5), the tokens added per second.
- now: optional function returning the current time in milliseconds; defaults to Date.now. Tests inject it.
Invalid capacity or refillPerSecond (missing, not a number, zero, negative, NaN, Infinity; capacity also must be an integer) throws a RangeError from createLimiter.

The returned limiter has:
- tryRemove(n = 1): refills first (tokens grow continuously with elapsed time, never above capacity), then if at least n tokens are available removes them and returns true, otherwise removes nothing and returns false. n must be a positive integer, otherwise RangeError. If n > capacity it can never succeed: throw a RangeError.
- available(): refills, then returns the current number of tokens rounded down to an integer.
- msUntil(n = 1): refills, then returns the whole number of milliseconds (rounded up) until n tokens will be available; 0 if they already are. Same RangeError rules for n as tryRemove.
- reset(): makes the bucket full again.

If now() ever returns a time earlier than the last time seen (the clock went backwards), no tokens are added and no tokens are lost; the earlier time becomes the new reference.

Add tests in test/limiter.test.js using node:test and node:assert. Touch no other file.
