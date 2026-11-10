const test = require('node:test');
const assert = require('node:assert');
const { createLimiter } = require('../src/limiter.js');

function clock(start = 0) { let t = start; const now = () => t; now.advance = (ms) => { t += ms; }; now.set = (ms) => { t = ms; }; return now; }

test('starts full and removes', () => {
  const now = clock(); const l = createLimiter({ capacity: 3, refillPerSecond: 1, now });
  assert.strictEqual(l.available(), 3);
  assert.strictEqual(l.tryRemove(), true);
  assert.strictEqual(l.tryRemove(2), true);
  assert.strictEqual(l.tryRemove(), false);
  assert.strictEqual(l.available(), 0);
});
test('failed remove takes nothing', () => {
  const now = clock(); const l = createLimiter({ capacity: 5, refillPerSecond: 1, now });
  l.tryRemove(4);
  assert.strictEqual(l.tryRemove(3), false);
  assert.strictEqual(l.available(), 1);
});
test('refills continuously', () => {
  const now = clock(); const l = createLimiter({ capacity: 10, refillPerSecond: 2, now });
  l.tryRemove(10);
  now.advance(250); assert.strictEqual(l.available(), 0);
  now.advance(250); assert.strictEqual(l.available(), 1);
  now.advance(1000); assert.strictEqual(l.available(), 3);
});
test('fractional refill rate accumulates', () => {
  const now = clock(); const l = createLimiter({ capacity: 2, refillPerSecond: 0.5, now });
  l.tryRemove(2);
  now.advance(1000); assert.strictEqual(l.tryRemove(), false);
  now.advance(1000); assert.strictEqual(l.tryRemove(), true);
});
test('never above capacity', () => {
  const now = clock(); const l = createLimiter({ capacity: 4, refillPerSecond: 100, now });
  l.tryRemove(1); now.advance(60000);
  assert.strictEqual(l.available(), 4);
});
test('msUntil rounds up and is 0 when available', () => {
  const now = clock(); const l = createLimiter({ capacity: 3, refillPerSecond: 3, now });
  assert.strictEqual(l.msUntil(3), 0);
  l.tryRemove(3);
  assert.strictEqual(l.msUntil(1), 334);
  assert.strictEqual(l.msUntil(3), 1000);
  now.advance(334); assert.strictEqual(l.msUntil(1), 0);
});
test('reset fills the bucket', () => {
  const now = clock(); const l = createLimiter({ capacity: 3, refillPerSecond: 1, now });
  l.tryRemove(3); l.reset(); assert.strictEqual(l.available(), 3);
});
test('clock going backwards adds and loses nothing', () => {
  const now = clock(10000); const l = createLimiter({ capacity: 10, refillPerSecond: 1, now });
  l.tryRemove(5);
  now.set(5000); assert.strictEqual(l.available(), 5);
  now.set(7000); assert.strictEqual(l.available(), 7);
});
test('n validation', () => {
  const l = createLimiter({ capacity: 3, refillPerSecond: 1, now: clock() });
  for (const bad of [0, -1, 1.5, NaN, '2']) {
    assert.throws(() => l.tryRemove(bad), RangeError, String(bad));
    assert.throws(() => l.msUntil(bad), RangeError, String(bad));
  }
  assert.throws(() => l.tryRemove(4), RangeError);
  assert.throws(() => l.msUntil(4), RangeError);
});
test('options validation', () => {
  const now = clock();
  for (const capacity of [0, -1, 1.5, NaN, Infinity, undefined, '3'])
    assert.throws(() => createLimiter({ capacity, refillPerSecond: 1, now }), RangeError, String(capacity));
  for (const refillPerSecond of [0, -1, NaN, Infinity, undefined, '1'])
    assert.throws(() => createLimiter({ capacity: 1, refillPerSecond, now }), RangeError, String(refillPerSecond));
});
test('defaults to Date.now', () => {
  const l = createLimiter({ capacity: 1, refillPerSecond: 1 });
  assert.strictEqual(l.tryRemove(), true);
  assert.strictEqual(l.tryRemove(), false);
});
test('independent limiters', () => {
  const now = clock();
  const a = createLimiter({ capacity: 1, refillPerSecond: 1, now }); const b = createLimiter({ capacity: 1, refillPerSecond: 1, now });
  a.tryRemove(); assert.strictEqual(b.available(), 1);
});
