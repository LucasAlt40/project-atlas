Bug report from customers about src/cart.js:

- A cart with 3 units of a product at 19.90 shows a total of 19.9 instead of 59.7.
- With the coupon TENOFF, a cart worth 5.00 shows a total of -5.
- With the coupon OFF20 (20% off), a cart worth 100 shows 20 instead of 80.
- Some totals show long decimals such as 0.30000000000000004.

Totals must be numbers rounded to cents (2 decimals) and never negative. Unknown coupons are ignored. Find the root cause and fix it in src/cart.js, keeping the existing API (module.exports = { total }). The existing tests in test/cart.test.js must keep passing; add a regression test there.
